// See the note in `options.rs`: `!(x > 0.0)` is the NaN-safe positivity check.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

//! Interest-rate primitives: continuously-compounded discount factors and
//! cost-of-carry forward prices.

/// Continuously-compounded discount factor `exp(-r * t)`.
///
/// Mirrors the convention used throughout the Black-Scholes implementation.
pub fn discount_factor(rate: f64, t: f64) -> f64 {
    libm::exp(-rate * t)
}

/// Cost-of-carry forward price for a non-dividend-paying asset:
/// `F = S * exp(r * t)`.
///
/// `spot <= 0.0` yields `NaN`.
pub fn forward_price(spot: f64, r: f64, t: f64) -> f64 {
    if !(spot > 0.0) {
        return f64::NAN;
    }
    spot * libm::exp(r * t)
}
