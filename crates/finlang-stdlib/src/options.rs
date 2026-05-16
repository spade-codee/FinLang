// We use the `!(x > 0.0)` idiom rather than `x <= 0.0` because the former
// correctly captures NaN inputs (NaN > 0.0 is false). `x <= 0.0` would let
// NaN slip through. This is the textbook way to validate "strictly positive
// real" inputs in numerical code, hence the local allow.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

//! Black-Scholes European option pricing, Greeks, and implied volatility.
//!
//! # Conventions
//!
//! * `r` is a **continuously-compounded** risk-free rate.
//! * `vol` is the annualised lognormal volatility.
//! * `t` is time to expiry in **years** (ACT/365 is the caller's problem).
//! * Greeks are returned in their natural (per-unit) form, **not** scaled to
//!   "per 1%" or "per day":
//!   * Vega is `dPrice / dVol` (multiply by `0.01` to get "per 1 vol point").
//!   * Theta is `dPrice / dt` per year (divide by 365 to get "per day").
//!   * Rho is `dPrice / dr` per unit rate (multiply by `0.01` to get "per 1%").
//!
//! # Edge cases
//!
//! * `spot <= 0.0` -> `NaN` (lognormal model is undefined for non-positive
//!   spot).
//! * `t <= 0.0` or `vol <= 0.0` -> price collapses to intrinsic value; Greeks
//!   collapse to their degenerate limits (delta is the in-the-money
//!   indicator; gamma, vega, theta, rho all zero).

use crate::normal::{norm_cdf, norm_pdf};
use crate::OptionType;

/// Compute `d1` for the Black-Scholes formula. Assumes `vol > 0`, `t > 0`,
/// `spot > 0`, `strike > 0`.
#[inline]
fn d1(spot: f64, strike: f64, vol: f64, r: f64, t: f64) -> f64 {
    (libm::log(spot / strike) + (r + 0.5 * vol * vol) * t) / (vol * libm::sqrt(t))
}

/// Compute `d2 = d1 - vol*sqrt(t)`.
#[inline]
fn d2_from_d1(d1: f64, vol: f64, t: f64) -> f64 {
    d1 - vol * libm::sqrt(t)
}

/// Intrinsic value `max(spot - strike, 0)` for a call, `max(strike - spot, 0)`
/// for a put.
#[inline]
fn intrinsic(spot: f64, strike: f64, opt: OptionType) -> f64 {
    match opt {
        OptionType::Call => (spot - strike).max(0.0),
        OptionType::Put => (strike - spot).max(0.0),
    }
}

/// Black-Scholes price of a European option.
pub fn black_scholes(spot: f64, strike: f64, vol: f64, r: f64, t: f64, opt: OptionType) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) || !(vol > 0.0) {
        return intrinsic(spot, strike, opt);
    }
    let d1 = d1(spot, strike, vol, r, t);
    let d2 = d2_from_d1(d1, vol, t);
    let disc = libm::exp(-r * t);
    match opt {
        OptionType::Call => spot * norm_cdf(d1) - strike * disc * norm_cdf(d2),
        OptionType::Put => strike * disc * norm_cdf(-d2) - spot * norm_cdf(-d1),
    }
}

/// Black-Scholes delta: `dPrice / dSpot`.
pub fn bs_delta(spot: f64, strike: f64, vol: f64, r: f64, t: f64, opt: OptionType) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) || !(vol > 0.0) {
        // Limit as t->0 or vol->0: delta is the in-the-money indicator. At the
        // strike we return 0.5 for the call / -0.5 for the put as a symmetric
        // convention; this branch is rarely hit in practice.
        return match opt {
            OptionType::Call => {
                if spot > strike {
                    1.0
                } else if spot < strike {
                    0.0
                } else {
                    0.5
                }
            }
            OptionType::Put => {
                if spot < strike {
                    -1.0
                } else if spot > strike {
                    0.0
                } else {
                    -0.5
                }
            }
        };
    }
    let d1 = d1(spot, strike, vol, r, t);
    match opt {
        OptionType::Call => norm_cdf(d1),
        OptionType::Put => norm_cdf(d1) - 1.0,
    }
}

/// Black-Scholes gamma: `d^2 Price / dSpot^2`. Identical for calls and puts.
pub fn bs_gamma(spot: f64, strike: f64, vol: f64, r: f64, t: f64) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) || !(vol > 0.0) {
        return 0.0;
    }
    let d1 = d1(spot, strike, vol, r, t);
    norm_pdf(d1) / (spot * vol * libm::sqrt(t))
}

/// Black-Scholes vega: `dPrice / dVol`. Identical for calls and puts. Returned
/// in price units per unit vol (multiply by `0.01` for "per vol point").
pub fn bs_vega(spot: f64, strike: f64, vol: f64, r: f64, t: f64) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) || !(vol > 0.0) {
        return 0.0;
    }
    let d1 = d1(spot, strike, vol, r, t);
    spot * norm_pdf(d1) * libm::sqrt(t)
}

/// Black-Scholes theta: `dPrice / dt`. Per year — divide by `365.0` for "per
/// calendar day".
pub fn bs_theta(spot: f64, strike: f64, vol: f64, r: f64, t: f64, opt: OptionType) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) || !(vol > 0.0) {
        return 0.0;
    }
    let d1 = d1(spot, strike, vol, r, t);
    let d2 = d2_from_d1(d1, vol, t);
    let disc = libm::exp(-r * t);
    let first_term = -spot * norm_pdf(d1) * vol / (2.0 * libm::sqrt(t));
    match opt {
        OptionType::Call => first_term - r * strike * disc * norm_cdf(d2),
        OptionType::Put => first_term + r * strike * disc * norm_cdf(-d2),
    }
}

/// Black-Scholes rho: `dPrice / dr`. Per unit rate — multiply by `0.01` for
/// "per 1% rate move".
pub fn bs_rho(spot: f64, strike: f64, vol: f64, r: f64, t: f64, opt: OptionType) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) {
        return f64::NAN;
    }
    if !(t > 0.0) {
        return 0.0;
    }
    if !(vol > 0.0) {
        // With t > 0 but vol = 0 the option is a deterministic forward; rho
        // is well-defined but Greeks above zero this out for simplicity.
        return 0.0;
    }
    let d1 = d1(spot, strike, vol, r, t);
    let d2 = d2_from_d1(d1, vol, t);
    let disc = libm::exp(-r * t);
    match opt {
        OptionType::Call => strike * t * disc * norm_cdf(d2),
        OptionType::Put => -strike * t * disc * norm_cdf(-d2),
    }
}

/// Newton-Raphson implied volatility solver.
///
/// Starts from the Brenner-Subrahmanyam initial guess
/// `sigma_0 = sqrt(2*pi/t) * market_price / spot`, then iterates
/// `sigma_{n+1} = sigma_n - (price(sigma_n) - market) / vega(sigma_n)`. Caps
/// each step at +/- 0.5 to avoid Newton overshoot blowing past the root, and
/// floors the candidate at `1e-12` to stay in the support of the model.
///
/// Returns `NaN` after 100 iterations without convergence, on negative
/// market price, or if vega collapses to zero (vertical pricing curve).
pub fn implied_vol(
    market_price: f64,
    spot: f64,
    strike: f64,
    r: f64,
    t: f64,
    opt: OptionType,
) -> f64 {
    if !(spot > 0.0) || !(strike > 0.0) || !(t > 0.0) || !(market_price >= 0.0) {
        return f64::NAN;
    }
    // No-arbitrage lower bound: option must be at least worth its intrinsic
    // discounted forward value. If market < intrinsic, Newton will diverge.
    let lower_bound = match opt {
        OptionType::Call => (spot - strike * libm::exp(-r * t)).max(0.0),
        OptionType::Put => (strike * libm::exp(-r * t) - spot).max(0.0),
    };
    if market_price < lower_bound - 1e-12 {
        return f64::NAN;
    }

    let mut sigma = libm::sqrt(2.0 * core::f64::consts::PI / t) * market_price / spot;
    if !(sigma > 0.0) || !sigma.is_finite() {
        sigma = 0.2;
    }

    let tol = 1e-8;
    for _ in 0..100 {
        let price = black_scholes(spot, strike, sigma, r, t, opt);
        let diff = price - market_price;
        if libm::fabs(diff) < tol {
            return sigma;
        }
        let vega = bs_vega(spot, strike, sigma, r, t);
        if !(vega > 1e-14) {
            return f64::NAN;
        }
        // Damp pathological steps. Vol moves of >0.5 per iter mean we are
        // far from the root and Newton is unreliable; clamp to keep iterates
        // sane.
        let step = (diff / vega).clamp(-0.5, 0.5);
        sigma -= step;
        if sigma < 1e-12 {
            sigma = 1e-12;
        }
    }
    f64::NAN
}
