//! End-to-end JIT tests: lex → parse → typecheck → lower → JIT → run.
//!
//! Each test compiles one of the three example `.fin` files and asserts the
//! returned scalar matches the reference value computed by calling the stdlib
//! functions directly from Rust.

mod common;

use finlang_codegen::{JitEngine, ScalarValue};
use finlang_stdlib::{black_scholes, bond_price, bs_delta, OptionType};

// ── option_pricing.fin ────────────────────────────────────────────────────────

#[test]
fn jit_option_pricing() {
    // option_pricing.fin returns `call_price` which is:
    //   black_scholes(105.0, 100.0, 0.20, 0.05, 0.5, Call)
    let expected = black_scholes(105.0, 100.0, 0.20, 0.05, 0.5, OptionType::Call);

    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/option_pricing.fin"),
    )
    .expect("read option_pricing.fin");

    let prog = common::compile_source(&src);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();

    println!("option_pricing JIT result: {result:?}");
    println!("expected:                  {expected:.6}");

    if let ScalarValue::F64(v) = result {
        assert!(
            (v - expected).abs() < 1e-4,
            "option_pricing: expected {expected:.6}, got {v:.6}"
        );
    } else {
        panic!("expected ScalarValue::F64, got {result:?}");
    }
}

// ── bond_portfolio.fin ────────────────────────────────────────────────────────

#[test]
fn jit_bond_portfolio() {
    // bond_portfolio.fin returns `portfolio` which is:
    //   bond_price(1000.0, 0.05, 0.04, 10) + bond_price(10000.0, 0.03, 0.035, 20)
    let bond_a = bond_price(1000.0, 0.05, 0.04, 10);
    let bond_b = bond_price(10000.0, 0.03, 0.035, 20);
    let expected = bond_a + bond_b;

    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bond_portfolio.fin"),
    )
    .expect("read bond_portfolio.fin");

    let prog = common::compile_source(&src);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();

    println!("bond_portfolio JIT result: {result:?}");
    println!("bond_a_pv:                 {bond_a:.6}");
    println!("bond_b_pv:                 {bond_b:.6}");
    println!("expected portfolio:        {expected:.6}");

    if let ScalarValue::F64(v) = result {
        assert!(
            (v - expected).abs() < 1e-4,
            "bond_portfolio: expected {expected:.6}, got {v:.6}"
        );
    } else {
        panic!("expected ScalarValue::F64, got {result:?}");
    }
}

// ── var_calculation.fin ───────────────────────────────────────────────────────

#[test]
fn jit_var_calculation() {
    // var_calculation.fin returns `var_95` which is:
    //   (position * delta) * move_size
    // where:
    //   delta     = bs_delta(100.0, 100.0, 0.20, 0.03, 0.25, Call)
    //   position  = 1000.0
    //   move_size = vol * z_95 = 0.20 * 1.645
    let delta = bs_delta(100.0, 100.0, 0.20, 0.03, 0.25, OptionType::Call);
    let position = 1000.0_f64;
    let move_size = 0.20_f64 * 1.645_f64;
    let expected = (position * delta) * move_size;

    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/var_calculation.fin"),
    )
    .expect("read var_calculation.fin");

    let prog = common::compile_source(&src);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();

    println!("var_calculation JIT result: {result:?}");
    println!("delta:                      {delta:.6}");
    println!("expected var_95:            {expected:.6}");

    if let ScalarValue::F64(v) = result {
        assert!(
            (v - expected).abs() < 1e-4,
            "var_calculation: expected {expected:.6}, got {v:.6}"
        );
    } else {
        panic!("expected ScalarValue::F64, got {result:?}");
    }
}
