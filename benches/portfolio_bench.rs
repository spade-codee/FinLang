//! End-to-end FinLang throughput benchmarks.
//!
//! The benchmark targets answer two distinct questions:
//!
//! 1. **JIT throughput** — how fast is the generated machine code at running
//!    a finished FinLang program?  Reported as nanoseconds per call to
//!    `JitProgram::run()` after a single up-front compilation.
//! 2. **Compilation throughput** — how expensive is the full
//!    source→native pipeline?  Reported as nanoseconds per `compile`
//!    invocation of the entire stack (parse → typecheck → lower →
//!    const_fold → dce → Cranelift JIT → finalize).
//!
//! A parallel safe-Rust baseline runs the same calculation by calling the
//! `finlang-stdlib` functions directly.  Because the JIT calls the same
//! `#[no_mangle]` symbols at runtime, this is the fairest possible
//! native-vs-native comparison; the JIT pays for one extra indirect call
//! plus its register-allocated arithmetic wrapper.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use finlang_codegen::{JitEngine, ScalarValue};
use finlang_ir::{const_fold, dce, lower, validate_ssa, IrProgram};
use finlang_parser::parse_str;
use finlang_stdlib::{black_scholes, bond_price, bs_delta, OptionType};
use finlang_types::check;

// ── Source fixtures ────────────────────────────────────────────────────────────

const OPTION_PRICING: &str = include_str!("../examples/option_pricing.fin");
const BOND_PORTFOLIO: &str = include_str!("../examples/bond_portfolio.fin");
const VAR_CALCULATION: &str = include_str!("../examples/var_calculation.fin");

// ── Pipeline helper ────────────────────────────────────────────────────────────

/// Run the front-end and IR optimisation passes.  Panics on any error — the
/// fixtures are known-good so an error here is a real test failure.
fn build_ir(source: &str) -> IrProgram {
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

/// JIT-compile an [`IrProgram`] and return the runnable program plus the
/// engine (kept alive so the code pages aren't unmapped).
fn jit(program: &IrProgram) -> (JitEngine, finlang_codegen::JitProgram) {
    let mut engine = JitEngine::new().expect("jit engine");
    let compiled = engine.compile(program).expect("compile");
    (engine, compiled)
}

// ── JIT-run throughput ─────────────────────────────────────────────────────────

fn bench_jit_run(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_run");
    group.throughput(Throughput::Elements(1));

    let (_engine_o, prog_o) = jit(&build_ir(OPTION_PRICING));
    group.bench_function("option_pricing", |b| {
        b.iter(|| black_box(prog_o.run()));
    });

    let (_engine_b, prog_b) = jit(&build_ir(BOND_PORTFOLIO));
    group.bench_function("bond_portfolio", |b| {
        b.iter(|| black_box(prog_b.run()));
    });

    let (_engine_v, prog_v) = jit(&build_ir(VAR_CALCULATION));
    group.bench_function("var_calculation", |b| {
        b.iter(|| black_box(prog_v.run()));
    });

    group.finish();
}

// ── Full pipeline (parse → JIT) ────────────────────────────────────────────────

fn bench_full_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_compile");
    group.sample_size(20); // JIT setup dominates; fewer samples is fine

    group.bench_function("option_pricing", |b| {
        b.iter(|| {
            let ir = build_ir(OPTION_PRICING);
            let (_engine, prog) = jit(&ir);
            black_box(prog.run());
        });
    });

    group.bench_function("bond_portfolio", |b| {
        b.iter(|| {
            let ir = build_ir(BOND_PORTFOLIO);
            let (_engine, prog) = jit(&ir);
            black_box(prog.run());
        });
    });

    group.finish();
}

// ── Safe-Rust baseline ─────────────────────────────────────────────────────────

fn bench_native_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_baseline");
    group.throughput(Throughput::Elements(1));

    group.bench_function("option_pricing", |b| {
        b.iter(|| {
            let spot = black_box(105.0f64);
            let strike = black_box(100.0f64);
            let vol = black_box(0.20f64);
            let r = black_box(0.05f64);
            let t = black_box(0.5f64);
            let price = black_scholes(spot, strike, vol, r, t, OptionType::Call);
            let delta = bs_delta(spot, strike, vol, r, t, OptionType::Call);
            black_box(price + delta);
        });
    });

    group.bench_function("bond_portfolio", |b| {
        b.iter(|| {
            let a = bond_price(
                black_box(1000.0),
                black_box(0.05),
                black_box(0.04),
                black_box(10),
            );
            let b_ = bond_price(
                black_box(10000.0),
                black_box(0.03),
                black_box(0.035),
                black_box(20),
            );
            black_box(a + b_);
        });
    });

    group.finish();
}

// ── Sanity check: JIT output matches native ────────────────────────────────────
//
// Not a bench — a one-shot precondition that runs once before Criterion
// takes over.  If the JIT value diverges from the safe-Rust value the
// benchmark numbers below would be meaningless, so we abort.

fn sanity_check() {
    let (_e, p) = jit(&build_ir(OPTION_PRICING));
    let v = match p.run() {
        ScalarValue::F64(v) => v,
        other => panic!("expected F64, got {other:?}"),
    };
    let native = black_scholes(105.0, 100.0, 0.20, 0.05, 0.5, OptionType::Call);
    let drift = (v - native).abs();
    assert!(drift < 1e-9, "JIT/native drift on option_pricing: {drift}");
}

fn benches(c: &mut Criterion) {
    sanity_check();
    bench_jit_run(c);
    bench_full_compile(c);
    bench_native_baseline(c);
}

criterion_group!(portfolio, benches);
criterion_main!(portfolio);
