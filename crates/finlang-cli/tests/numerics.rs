//! Numerical-precision integration tests.
//!
//! Drives the full library pipeline (parse → typecheck → lower → optimise →
//! JIT) on every example program and asserts that the JIT result matches a
//! safe-Rust baseline computed by calling the same `finlang-stdlib`
//! functions directly.
//!
//! These tests are intentionally library-only (no `Command::spawn` overhead)
//! so they can re-use a single fixture set across multiple assertions and
//! still finish in milliseconds.

use finlang_codegen::{JitEngine, ScalarValue};
use finlang_ir::{const_fold, dce, lower, validate_ssa, IrProgram};
use finlang_parser::parse_str;
use finlang_stdlib::{black_scholes, bond_price, bs_delta, OptionType};
use finlang_types::check;

const OPTION_PRICING: &str = include_str!("../../../examples/option_pricing.fin");
const BOND_PORTFOLIO: &str = include_str!("../../../examples/bond_portfolio.fin");
const VAR_CALCULATION: &str = include_str!("../../../examples/var_calculation.fin");

/// Lower, optimise, and validate a source string into a runnable IR program.
fn build(source: &str) -> IrProgram {
    let parsed = parse_str(source);
    assert!(parsed.errors.is_empty(), "parse errors: {:?}", parsed.errors);
    let types = check(&parsed.items);
    assert!(types.errors.is_empty(), "type errors: {:?}", types.errors);
    let mut prog = lower(&parsed.items, &types).expect("lower");
    const_fold(&mut prog);
    dce(&mut prog);
    validate_ssa(&prog).expect("ssa invariants");
    prog
}

/// JIT-compile `program` and run it once, returning the f64 result.  Panics
/// when the program returns a non-f64 value.
fn jit_run_f64(program: &IrProgram) -> f64 {
    let mut engine = JitEngine::new().expect("jit engine");
    let compiled = engine.compile(program).expect("compile");
    match compiled.run() {
        ScalarValue::F64(v) => v,
        other => panic!("expected F64, got {other:?}"),
    }
}

#[test]
fn option_pricing_jit_matches_native() {
    let jit = jit_run_f64(&build(OPTION_PRICING));
    let native = black_scholes(105.0, 100.0, 0.20, 0.05, 0.5, OptionType::Call);
    let drift = (jit - native).abs();
    assert!(
        drift < 1e-9,
        "JIT={jit} vs native={native}, drift={drift}"
    );
}

#[test]
fn bond_portfolio_jit_matches_native() {
    let jit = jit_run_f64(&build(BOND_PORTFOLIO));
    let a = bond_price(1000.0, 0.05, 0.04, 10);
    let b = bond_price(10000.0, 0.03, 0.035, 20);
    let native = a + b;
    let drift = (jit - native).abs();
    assert!(
        drift < 1e-6,
        "JIT={jit} vs native={native}, drift={drift}"
    );
}

#[test]
fn var_calculation_jit_matches_native() {
    // var_95 = (position * delta) * (vol * z_95) where:
    //   position = 1000.0, vol = 0.20, z_95 = 1.645
    //   delta    = bs_delta(100, 100, 0.20, 0.03, 0.25, Call)
    let jit = jit_run_f64(&build(VAR_CALCULATION));
    let delta = bs_delta(100.0, 100.0, 0.20, 0.03, 0.25, OptionType::Call);
    let move_size = 0.20 * 1.645;
    let native = (1000.0 * delta) * move_size;
    let drift = (jit - native).abs();
    assert!(
        drift < 1e-6,
        "JIT={jit} vs native={native}, drift={drift}"
    );
}

#[test]
fn jit_results_are_deterministic_across_runs() {
    // Compile-and-run twice; floating-point ops should be bit-identical.
    let prog = build(OPTION_PRICING);
    let first = jit_run_f64(&prog);
    let second = jit_run_f64(&prog);
    assert_eq!(first.to_bits(), second.to_bits(), "non-deterministic JIT");
}

#[test]
fn optimised_ir_is_smaller_than_unoptimised() {
    // Re-build without const_fold/dce to size against the optimised version.
    let parsed = parse_str(OPTION_PRICING);
    let types = check(&parsed.items);
    let unopt = lower(&parsed.items, &types).expect("lower");
    let mut opt = unopt.clone();
    const_fold(&mut opt);
    dce(&mut opt);

    let unopt_insts: usize = unopt
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .map(|b| b.insts.len())
        .sum();
    let opt_insts: usize = opt
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .map(|b| b.insts.len())
        .sum();
    assert!(
        opt_insts <= unopt_insts,
        "optimisation increased size: {unopt_insts} → {opt_insts}"
    );
}
