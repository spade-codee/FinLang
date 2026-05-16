// See the note in `options.rs`: `!(x > 0.0)` is the NaN-safe positivity check.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

//! Fixed-income primitives: bond price, Macaulay duration, PV01.
//!
//! # Conventions
//!
//! * **Annual discrete compounding.** A bond with `periods = N` pays a single
//!   coupon at the end of each of `N` years, plus face value at year `N`. The
//!   discount factor for cashflow at year `k` is `1 / (1 + ytm)^k`.
//! * `coupon` is the annual coupon **rate** (e.g. `0.05` for a 5% coupon).
//!   The cashflow paid each year is `face * coupon`.
//! * `ytm` is the yield to maturity, annual, decimal.
//! * Duration is **Macaulay** duration in years.
//! * PV01 (a.k.a. DV01) is the price change in face-value currency for a 1bp
//!   **increase** in yield, returned as a **positive** number — by long-bond
//!   convention `PV01 = -dP/dy * 0.0001`.
//!
//! # Edge cases
//!
//! * `periods <= 0` -> `NaN`.
//! * `1 + ytm <= 0` -> `NaN` (the discount factor would be undefined / explode).

/// Present value of a vanilla bullet bond.
///
/// `P = sum_{k=1}^N C / (1+y)^k + F / (1+y)^N` where `C = face * coupon`.
pub fn bond_price(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    if periods <= 0 {
        return f64::NAN;
    }
    let one_plus_y = 1.0 + ytm;
    if !(one_plus_y > 0.0) {
        return f64::NAN;
    }
    let c = face * coupon;
    let n = periods as f64;
    // Closed-form annuity: sum_{k=1}^N (1+y)^{-k} = (1 - (1+y)^{-N}) / y, with
    // the y=0 limit handled separately to avoid 0/0.
    let pv_coupons = if ytm == 0.0 {
        c * n
    } else {
        c * (1.0 - libm::pow(one_plus_y, -n)) / ytm
    };
    let pv_face = face * libm::pow(one_plus_y, -n);
    pv_coupons + pv_face
}

/// Macaulay duration in years.
///
/// `D = (sum_k k * PV(CF_k)) / P`. For a zero-coupon bond, this collapses to
/// `periods`.
pub fn bond_duration(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    if periods <= 0 {
        return f64::NAN;
    }
    let one_plus_y = 1.0 + ytm;
    if !(one_plus_y > 0.0) {
        return f64::NAN;
    }
    let c = face * coupon;
    let mut weighted_pv = 0.0_f64;
    let mut total_pv = 0.0_f64;
    for k in 1..=periods {
        let kf = k as f64;
        let df = libm::pow(one_plus_y, -kf);
        let mut cf = c;
        if k == periods {
            cf += face;
        }
        let pv = cf * df;
        total_pv += pv;
        weighted_pv += kf * pv;
    }
    if total_pv == 0.0 {
        return f64::NAN;
    }
    weighted_pv / total_pv
}

/// PV01: price sensitivity to a 1bp increase in yield, returned positive for a
/// long bond position. Computed analytically as `-dP/dy * 1e-4`.
///
/// `dP/dy = -sum_k k * CF_k / (1+y)^{k+1}`.
pub fn pv01(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    if periods <= 0 {
        return f64::NAN;
    }
    let one_plus_y = 1.0 + ytm;
    if !(one_plus_y > 0.0) {
        return f64::NAN;
    }
    let c = face * coupon;
    // dP/dy = -sum_k k * CF_k / (1+y)^(k+1).
    let mut dp_dy = 0.0_f64;
    for k in 1..=periods {
        let kf = k as f64;
        let mut cf = c;
        if k == periods {
            cf += face;
        }
        dp_dy -= kf * cf * libm::pow(one_plus_y, -(kf + 1.0));
    }
    -dp_dy * 1e-4
}
