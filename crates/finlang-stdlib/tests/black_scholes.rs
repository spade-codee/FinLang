//! Hull §15.9 reference values plus put-call parity and intrinsic-value
//! edge cases for the Black-Scholes implementation.

use finlang_stdlib::{
    black_scholes, bs_delta, bs_gamma, bs_rho, bs_theta, bs_vega, OptionType,
};

const PRICE_EPS: f64 = 1e-4;
const GREEK_EPS: f64 = 1e-4;
const PARITY_EPS: f64 = 1e-10;

// Hull, "Options, Futures, and Other Derivatives", §15.9:
//   spot=42, strike=40, vol=0.20, r=0.10, t=0.5
const SPOT: f64 = 42.0;
const STRIKE: f64 = 40.0;
const VOL: f64 = 0.20;
const RATE: f64 = 0.10;
const TIME: f64 = 0.5;

#[test]
fn hull_call_price() {
    let p = black_scholes(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Call);
    assert!((p - 4.7594).abs() < PRICE_EPS, "got {p}");
}

#[test]
fn hull_put_price() {
    let p = black_scholes(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Put);
    assert!((p - 0.8086).abs() < PRICE_EPS, "got {p}");
}

#[test]
fn hull_call_delta() {
    let d = bs_delta(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Call);
    assert!((d - 0.7791).abs() < GREEK_EPS, "got {d}");
}

#[test]
fn hull_put_delta() {
    let d = bs_delta(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Put);
    assert!((d - (-0.2209)).abs() < GREEK_EPS, "got {d}");
}

#[test]
fn hull_gamma() {
    let g = bs_gamma(SPOT, STRIKE, VOL, RATE, TIME);
    assert!((g - 0.04996).abs() < GREEK_EPS, "got {g}");
}

#[test]
fn hull_vega() {
    // Hull rounds vega to 8.8133 (4dp). The true value is 8.81342; we
    // allow 5e-4 to absorb Hull's last-digit rounding.
    let v = bs_vega(SPOT, STRIKE, VOL, RATE, TIME);
    assert!((v - 8.8134).abs() < 5e-4, "got {v}");
}

#[test]
fn hull_theta_call() {
    // The Hull-convention theta (∂C/∂t with t = calendar time) for this
    // exact parameter set, cross-checked via the Black-Scholes PDE
    //   ∂V/∂t = rV - ½σ²S²Γ - rSΔ
    // evaluates to -4.5589. This is QuantLib-consistent. The textbook's
    // -4.31 figure belongs to a *different* example (S=49, K=50, T=0.3846).
    let th = bs_theta(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Call);
    assert!((th - (-4.5591)).abs() < 5e-4, "got {th}");
}

#[test]
fn hull_rho_call() {
    // K * T * exp(-rT) * N(d2) with d2 = 0.62783 -> N(d2) = 0.73493 ->
    // rho = 40 * 0.5 * 0.95123 * 0.73493 = 13.9820. The 13.9809 figure in
    // some references uses N(d2) rounded to 4dp.
    let rho = bs_rho(SPOT, STRIKE, VOL, RATE, TIME, OptionType::Call);
    assert!((rho - 13.9820).abs() < GREEK_EPS, "got {rho}");
}

/// Put-call parity: `Call - Put = Spot - Strike * exp(-r*t)`.
#[test]
fn put_call_parity_random_parameters() {
    let cases = [
        (100.0, 100.0, 0.20, 0.05, 1.0),
        (50.0, 60.0, 0.30, 0.03, 0.25),
        (120.0, 100.0, 0.15, 0.02, 2.0),
        (80.0, 95.0, 0.40, 0.07, 0.75),
        (200.0, 180.0, 0.25, 0.045, 1.5),
    ];
    for (s, k, v, r, t) in cases {
        let c = black_scholes(s, k, v, r, t, OptionType::Call);
        let p = black_scholes(s, k, v, r, t, OptionType::Put);
        let parity = s - k * (-r * t).exp();
        assert!(
            (c - p - parity).abs() < PARITY_EPS,
            "parity violated: c-p={} parity={} for ({s},{k},{v},{r},{t})",
            c - p,
            parity
        );
    }
}

#[test]
fn intrinsic_at_expiry_call_itm() {
    let p = black_scholes(110.0, 100.0, 0.2, 0.05, 0.0, OptionType::Call);
    assert!((p - 10.0).abs() < PARITY_EPS);
}

#[test]
fn intrinsic_at_expiry_call_otm() {
    let p = black_scholes(90.0, 100.0, 0.2, 0.05, 0.0, OptionType::Call);
    assert!(p.abs() < PARITY_EPS);
}

#[test]
fn intrinsic_at_expiry_put_itm() {
    let p = black_scholes(90.0, 100.0, 0.2, 0.05, 0.0, OptionType::Put);
    assert!((p - 10.0).abs() < PARITY_EPS);
}

#[test]
fn zero_vol_collapses_to_intrinsic() {
    // With vol = 0 and t > 0, the only thing that matters is whether S is
    // above the strike — but the model still returns intrinsic per the
    // documented convention.
    let p = black_scholes(110.0, 100.0, 0.0, 0.05, 1.0, OptionType::Call);
    assert!((p - 10.0).abs() < PARITY_EPS);
}

#[test]
fn greeks_zero_at_expiry() {
    assert_eq!(bs_gamma(100.0, 100.0, 0.2, 0.05, 0.0), 0.0);
    assert_eq!(bs_vega(100.0, 100.0, 0.2, 0.05, 0.0), 0.0);
    assert_eq!(
        bs_theta(100.0, 100.0, 0.2, 0.05, 0.0, OptionType::Call),
        0.0
    );
    assert_eq!(
        bs_rho(100.0, 100.0, 0.2, 0.05, 0.0, OptionType::Call),
        0.0
    );
}

#[test]
fn negative_spot_is_nan() {
    let p = black_scholes(-1.0, 100.0, 0.2, 0.05, 1.0, OptionType::Call);
    assert!(p.is_nan());
}
