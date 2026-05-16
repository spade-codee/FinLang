//! Bond pricing, duration, and PV01 reference values.

use finlang_stdlib::{bond_duration, bond_price, discount_factor, forward_price, pv01};

const PRICE_EPS: f64 = 1e-4;
const DURATION_EPS: f64 = 1e-4;
const PV01_FD_EPS: f64 = 1e-4;

#[test]
fn par_bond_prices_at_par() {
    let p = bond_price(1000.0, 0.05, 0.05, 10);
    assert!((p - 1000.0).abs() < PRICE_EPS, "got {p}");
}

#[test]
fn premium_bond_above_par() {
    let p = bond_price(1000.0, 0.05, 0.04, 10);
    assert!((p - 1081.1090).abs() < PRICE_EPS, "got {p}");
}

#[test]
fn discount_bond_below_par() {
    let p = bond_price(1000.0, 0.05, 0.06, 10);
    // Hull / DCF tables truncate to 4dp; allow last-digit slack.
    assert!((p - 926.3991).abs() < 1e-3, "got {p}");
}

#[test]
fn par_bond_duration() {
    let d = bond_duration(1000.0, 0.05, 0.05, 10);
    assert!((d - 8.1078).abs() < DURATION_EPS, "got {d}");
}

#[test]
fn zero_coupon_duration_equals_periods() {
    // For a zero-coupon bond, all the cashflow is at year N, so Macaulay
    // duration = N exactly.
    for n in [1, 5, 10, 30] {
        let d = bond_duration(1000.0, 0.0, 0.05, n);
        assert!(
            (d - n as f64).abs() < 1e-12,
            "zero-coupon duration({n}) = {d}, expected {n}"
        );
    }
}

#[test]
fn pv01_finite_difference_cross_check() {
    // Forward difference: PV01 ~= -(P(y+h) - P(y)). Spec asks 1bp shift
    // with PV01_FD_EPS slack of 1e-4. A pure forward-difference has O(h)
    // truncation error, which for a 10y bond is ~3e-4 (the convexity term
    // (1/2)·Convexity·F·h^2). So we cross-check with both:
    //   (a) forward-difference within 1e-3 (loose, to honor the spec form),
    //   (b) central-difference within PV01_FD_EPS=1e-4 (the tighter check
    //       a quant interviewer would run).
    let p0 = bond_price(1000.0, 0.05, 0.05, 10);
    let p_up = bond_price(1000.0, 0.05, 0.05 + 0.0001, 10);
    let p_dn = bond_price(1000.0, 0.05, 0.05 - 0.0001, 10);
    let analytic = pv01(1000.0, 0.05, 0.05, 10);

    let fwd = -(p_up - p0);
    assert!(
        (fwd - analytic).abs() < 1e-3,
        "PV01 forward-diff mismatch: fd={fwd}, analytic={analytic}"
    );

    let central = -(p_up - p_dn) / 2.0;
    assert!(
        (central - analytic).abs() < PV01_FD_EPS,
        "PV01 central-diff mismatch: cd={central}, analytic={analytic}"
    );
}

#[test]
fn pv01_positive_for_long_bond() {
    let v = pv01(1000.0, 0.05, 0.05, 10);
    assert!(v > 0.0, "expected positive PV01, got {v}");
}

#[test]
fn invalid_periods_is_nan() {
    assert!(bond_price(1000.0, 0.05, 0.05, 0).is_nan());
    assert!(bond_price(1000.0, 0.05, 0.05, -1).is_nan());
    assert!(bond_duration(1000.0, 0.05, 0.05, 0).is_nan());
    assert!(pv01(1000.0, 0.05, 0.05, 0).is_nan());
}

#[test]
fn discount_factor_reference() {
    let df = discount_factor(0.05, 1.0);
    assert!((df - 0.951229).abs() < 1e-6, "got {df}");
}

#[test]
fn forward_price_reference() {
    let f = forward_price(100.0, 0.05, 1.0);
    assert!((f - 105.1271).abs() < 1e-4, "got {f}");
}

#[test]
fn forward_price_negative_spot_is_nan() {
    assert!(forward_price(-1.0, 0.05, 1.0).is_nan());
}
