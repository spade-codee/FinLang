//! Verify the `extern "C"` ABI shims match the safe Rust wrappers, and that
//! an invalid `opt` discriminant returns `NaN` instead of panicking.

use finlang_stdlib::*;

const ABI_EPS: f64 = 1e-12;

fn eq_or_nan(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    (a - b).abs() < ABI_EPS
}

#[test]
fn extern_black_scholes_matches_safe() {
    let a = finlang_black_scholes(42.0, 40.0, 0.2, 0.10, 0.5, 0);
    let b = black_scholes(42.0, 40.0, 0.2, 0.10, 0.5, OptionType::Call);
    assert!(eq_or_nan(a, b), "{a} vs {b}");
}

#[test]
fn extern_delta_matches_safe() {
    let a = finlang_bs_delta(42.0, 40.0, 0.2, 0.10, 0.5, 1);
    let b = bs_delta(42.0, 40.0, 0.2, 0.10, 0.5, OptionType::Put);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_gamma_matches_safe() {
    let a = finlang_bs_gamma(42.0, 40.0, 0.2, 0.10, 0.5);
    let b = bs_gamma(42.0, 40.0, 0.2, 0.10, 0.5);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_vega_matches_safe() {
    let a = finlang_bs_vega(42.0, 40.0, 0.2, 0.10, 0.5);
    let b = bs_vega(42.0, 40.0, 0.2, 0.10, 0.5);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_theta_matches_safe() {
    let a = finlang_bs_theta(42.0, 40.0, 0.2, 0.10, 0.5, 0);
    let b = bs_theta(42.0, 40.0, 0.2, 0.10, 0.5, OptionType::Call);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_rho_matches_safe() {
    let a = finlang_bs_rho(42.0, 40.0, 0.2, 0.10, 0.5, 0);
    let b = bs_rho(42.0, 40.0, 0.2, 0.10, 0.5, OptionType::Call);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_implied_vol_matches_safe() {
    let price = black_scholes(100.0, 100.0, 0.25, 0.05, 1.0, OptionType::Call);
    let a = finlang_implied_vol(price, 100.0, 100.0, 0.05, 1.0, 0);
    let b = implied_vol(price, 100.0, 100.0, 0.05, 1.0, OptionType::Call);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_bond_price_matches_safe() {
    let a = finlang_bond_price(1000.0, 0.05, 0.05, 10);
    let b = bond_price(1000.0, 0.05, 0.05, 10);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_bond_duration_matches_safe() {
    let a = finlang_bond_duration(1000.0, 0.05, 0.05, 10);
    let b = bond_duration(1000.0, 0.05, 0.05, 10);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_pv01_matches_safe() {
    let a = finlang_pv01(1000.0, 0.05, 0.05, 10);
    let b = pv01(1000.0, 0.05, 0.05, 10);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_discount_factor_matches_safe() {
    let a = finlang_discount_factor(0.05, 1.0);
    let b = discount_factor(0.05, 1.0);
    assert!(eq_or_nan(a, b));
}

#[test]
fn extern_forward_price_matches_safe() {
    let a = finlang_forward_price(100.0, 0.05, 1.0);
    let b = forward_price(100.0, 0.05, 1.0);
    assert!(eq_or_nan(a, b));
}

// =============================================================================
// Invalid opt discriminant -> NaN.
// =============================================================================

#[test]
fn invalid_opt_black_scholes_is_nan() {
    assert!(finlang_black_scholes(42.0, 40.0, 0.2, 0.10, 0.5, 2).is_nan());
}

#[test]
fn invalid_opt_delta_is_nan() {
    assert!(finlang_bs_delta(42.0, 40.0, 0.2, 0.10, 0.5, 2).is_nan());
}

#[test]
fn invalid_opt_theta_is_nan() {
    assert!(finlang_bs_theta(42.0, 40.0, 0.2, 0.10, 0.5, 2).is_nan());
}

#[test]
fn invalid_opt_rho_is_nan() {
    assert!(finlang_bs_rho(42.0, 40.0, 0.2, 0.10, 0.5, 2).is_nan());
}

#[test]
fn invalid_opt_implied_vol_is_nan() {
    assert!(finlang_implied_vol(5.0, 100.0, 100.0, 0.05, 1.0, 2).is_nan());
}
