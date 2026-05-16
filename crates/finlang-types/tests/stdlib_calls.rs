//! Tests for stdlib function call type-checking.
//!
//! Each stdlib function is called with:
//!   1. Correct argument types → no errors.
//!   2. One wrong-typed argument → exactly one `MismatchedArgument` error.
//!   3. Wrong arity → exactly one `WrongArity` error.

use finlang_types::{check_str, TypeError};

fn ok(src: &str) {
    let r = check_str(src);
    assert!(
        r.errors.is_empty(),
        "expected no errors for:\n{src}\nbut got: {:#?}",
        r.errors
    );
}

fn one_wrong_arg(src: &str, expected_fn: &str, expected_idx: usize) {
    let r = check_str(src);
    assert_eq!(
        r.errors.len(),
        1,
        "expected exactly 1 error for:\n{src}\nbut got {}: {:#?}",
        r.errors.len(),
        r.errors
    );
    match &r.errors[0] {
        TypeError::MismatchedArgument { fn_name, arg_index, .. } => {
            assert_eq!(fn_name, expected_fn);
            assert_eq!(*arg_index, expected_idx);
        }
        other => panic!("expected MismatchedArgument, got {other:?}"),
    }
}

fn one_wrong_arity(src: &str, expected_fn: &str) {
    let r = check_str(src);
    assert_eq!(
        r.errors.len(),
        1,
        "expected exactly 1 arity error for:\n{src}\nbut got {}: {:#?}",
        r.errors.len(),
        r.errors
    );
    match &r.errors[0] {
        TypeError::WrongArity { fn_name, .. } => {
            assert_eq!(fn_name, expected_fn);
        }
        other => panic!("expected WrongArity, got {other:?}"),
    }
}

// Shared variable setup.
const SETUP: &str = "
let spot:   price    = 100.0 as price
let strike: price    = 100.0 as price
let vol:    rate     = 0.20
let r:      rate     = 0.03
let t:      years    = 0.25
let face:   notional = 1000.0 as notional
let coupon: rate     = 0.05
let ytm:    rate     = 0.04
let n:      int      = 10
let bp:     basis_points = 5.0 as basis_points
";

fn src(extra: &str) -> String {
    format!("{SETUP}\n{extra}")
}

// ── black_scholes ─────────────────────────────────────────────────────────────

#[test]
fn black_scholes_correct() {
    ok(&src("black_scholes(spot, strike, vol, r, t, Call)"));
}

#[test]
fn black_scholes_wrong_arg0() {
    // Pass rate where price is expected.
    one_wrong_arg(
        &src("black_scholes(vol, strike, vol, r, t, Call)"),
        "black_scholes",
        0,
    );
}

#[test]
fn black_scholes_wrong_arity() {
    one_wrong_arity(&src("black_scholes(spot, strike, vol, r, t)"), "black_scholes");
}

// ── bs_delta ──────────────────────────────────────────────────────────────────

#[test]
fn bs_delta_correct() {
    ok(&src("bs_delta(spot, strike, vol, r, t, Call)"));
}

#[test]
fn bs_delta_wrong_arg3() {
    // Pass price where rate is expected.
    one_wrong_arg(
        &src("bs_delta(spot, strike, vol, spot, t, Call)"),
        "bs_delta",
        3,
    );
}

#[test]
fn bs_delta_wrong_arity() {
    one_wrong_arity(&src("bs_delta(spot)"), "bs_delta");
}

// ── bs_gamma ──────────────────────────────────────────────────────────────────

#[test]
fn bs_gamma_correct() {
    ok(&src("bs_gamma(spot, strike, vol, r, t)"));
}

#[test]
fn bs_gamma_wrong_arg2() {
    one_wrong_arg(
        &src("bs_gamma(spot, strike, spot, r, t)"),
        "bs_gamma",
        2,
    );
}

#[test]
fn bs_gamma_wrong_arity() {
    one_wrong_arity(&src("bs_gamma(spot, strike)"), "bs_gamma");
}

// ── bs_vega ───────────────────────────────────────────────────────────────────

#[test]
fn bs_vega_correct() {
    ok(&src("bs_vega(spot, strike, vol, r, t)"));
}

#[test]
fn bs_vega_wrong_arg4() {
    one_wrong_arg(
        &src("bs_vega(spot, strike, vol, r, vol)"),
        "bs_vega",
        4,
    );
}

// ── bs_theta ──────────────────────────────────────────────────────────────────

#[test]
fn bs_theta_correct() {
    ok(&src("bs_theta(spot, strike, vol, r, t, Put)"));
}

#[test]
fn bs_theta_wrong_arg5() {
    // Pass rate where option_type is expected.
    one_wrong_arg(
        &src("bs_theta(spot, strike, vol, r, t, vol)"),
        "bs_theta",
        5,
    );
}

// ── bs_rho ────────────────────────────────────────────────────────────────────

#[test]
fn bs_rho_correct() {
    ok(&src("bs_rho(spot, strike, vol, r, t, Call)"));
}

// ── implied_vol ───────────────────────────────────────────────────────────────

#[test]
fn implied_vol_correct() {
    ok(&src("implied_vol(spot, spot, strike, r, t, Call)"));
}

#[test]
fn implied_vol_wrong_arity() {
    one_wrong_arity(&src("implied_vol(spot, spot, strike, r, t)"), "implied_vol");
}

// ── bond_price ────────────────────────────────────────────────────────────────

#[test]
fn bond_price_correct() {
    ok(&src("bond_price(face, coupon, ytm, n)"));
}

#[test]
fn bond_price_wrong_arg0() {
    // Pass price where notional is expected.
    one_wrong_arg(
        &src("bond_price(spot, coupon, ytm, n)"),
        "bond_price",
        0,
    );
}

#[test]
fn bond_price_wrong_arg3() {
    // Pass rate where int is expected.
    one_wrong_arg(
        &src("bond_price(face, coupon, ytm, vol)"),
        "bond_price",
        3,
    );
}

#[test]
fn bond_price_wrong_arity() {
    one_wrong_arity(&src("bond_price(face, coupon, ytm)"), "bond_price");
}

// ── bond_duration ─────────────────────────────────────────────────────────────

#[test]
fn bond_duration_correct() {
    ok(&src("bond_duration(face, coupon, ytm, n)"));
}

// ── pv01 ──────────────────────────────────────────────────────────────────────

#[test]
fn pv01_correct() {
    ok(&src("pv01(face, coupon, ytm, n)"));
}

#[test]
fn pv01_wrong_arg2() {
    one_wrong_arg(
        &src("pv01(face, coupon, spot, n)"),
        "pv01",
        2,
    );
}

// ── discount_factor ───────────────────────────────────────────────────────────

#[test]
fn discount_factor_correct() {
    ok(&src("discount_factor(r, t)"));
}

#[test]
fn discount_factor_wrong_arity() {
    one_wrong_arity(&src("discount_factor(r)"), "discount_factor");
}

// ── forward_price ─────────────────────────────────────────────────────────────

#[test]
fn forward_price_correct() {
    ok(&src("forward_price(spot, r, t)"));
}

#[test]
fn forward_price_wrong_arg0() {
    one_wrong_arg(
        &src("forward_price(vol, r, t)"),
        "forward_price",
        0,
    );
}

// ── Unknown function ──────────────────────────────────────────────────────────

#[test]
fn unknown_function() {
    let r = check_str("nonexistent_fn(1.0 as price)");
    assert_eq!(r.errors.len(), 1);
    assert!(matches!(r.errors[0], TypeError::UnknownFunction { .. }));
}

// ── Return types are correct ──────────────────────────────────────────────────

#[test]
fn black_scholes_returns_price() {
    let src = format!("{SETUP}\nlet result: price = black_scholes(spot, strike, vol, r, t, Call)");
    ok(&src);
}

#[test]
fn bs_delta_returns_rate() {
    let src = format!("{SETUP}\nlet result: rate = bs_delta(spot, strike, vol, r, t, Call)");
    ok(&src);
}

#[test]
fn bond_price_returns_price() {
    let src = format!("{SETUP}\nlet result: price = bond_price(face, coupon, ytm, n)");
    ok(&src);
}

#[test]
fn bond_duration_returns_years() {
    let src = format!("{SETUP}\nlet result: years = bond_duration(face, coupon, ytm, n)");
    ok(&src);
}

#[test]
fn discount_factor_returns_rate() {
    let src = format!("{SETUP}\nlet result: rate = discount_factor(r, t)");
    ok(&src);
}
