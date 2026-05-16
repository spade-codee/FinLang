//! Round-trip tests for the Newton-Raphson implied-vol solver.

use finlang_stdlib::{black_scholes, implied_vol, OptionType};

const ROUND_TRIP_EPS: f64 = 1e-6;

fn check_round_trip(spot: f64, strike: f64, vol: f64, r: f64, t: f64, opt: OptionType) {
    let price = black_scholes(spot, strike, vol, r, t, opt);
    let iv = implied_vol(price, spot, strike, r, t, opt);
    assert!(
        (iv - vol).abs() < ROUND_TRIP_EPS,
        "round-trip failed: vol={vol} iv={iv} for ({spot},{strike},{r},{t},{opt:?})"
    );
}

#[test]
fn round_trip_atm_call() {
    check_round_trip(100.0, 100.0, 0.25, 0.05, 1.0, OptionType::Call);
}

#[test]
fn round_trip_atm_put() {
    check_round_trip(100.0, 100.0, 0.25, 0.05, 1.0, OptionType::Put);
}

#[test]
fn round_trip_itm_call() {
    check_round_trip(110.0, 100.0, 0.30, 0.04, 0.5, OptionType::Call);
}

#[test]
fn round_trip_otm_call() {
    check_round_trip(90.0, 100.0, 0.35, 0.03, 1.5, OptionType::Call);
}

#[test]
fn round_trip_itm_put() {
    check_round_trip(90.0, 100.0, 0.20, 0.02, 0.75, OptionType::Put);
}

#[test]
fn round_trip_otm_put() {
    check_round_trip(110.0, 100.0, 0.40, 0.06, 2.0, OptionType::Put);
}

#[test]
fn round_trip_low_vol_call() {
    check_round_trip(100.0, 100.0, 0.10, 0.05, 1.0, OptionType::Call);
}

#[test]
fn round_trip_high_vol_put() {
    check_round_trip(50.0, 60.0, 0.60, 0.05, 0.5, OptionType::Put);
}

#[test]
fn round_trip_short_dated() {
    check_round_trip(100.0, 100.0, 0.20, 0.05, 0.05, OptionType::Call);
}

#[test]
fn non_convergence_returns_nan() {
    // Arbitrage-violating: a deep-ITM call (S=200, K=100, r=0.05, t=1) is
    // worth at least S - K*exp(-r*t) ~ 200 - 95.12 = 104.88. A market price
    // of 0.01 is below the no-arbitrage lower bound and the solver should
    // refuse to "find" a vol.
    let iv = implied_vol(0.01, 200.0, 100.0, 0.05, 1.0, OptionType::Call);
    assert!(iv.is_nan(), "expected NaN, got {iv}");
}

#[test]
fn negative_market_price_is_nan() {
    let iv = implied_vol(-1.0, 100.0, 100.0, 0.05, 1.0, OptionType::Call);
    assert!(iv.is_nan());
}
