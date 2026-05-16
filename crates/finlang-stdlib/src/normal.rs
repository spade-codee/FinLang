//! Standard normal distribution helpers.
//!
//! These are deliberately implemented in terms of `libm::erf` and `libm::exp`
//! rather than `f64::erf`/`f64::exp` so that values are bit-for-bit
//! reproducible across host platforms. Cranelift-JITed FinLang code calls into
//! the same `libm` routines, which is the property the rest of the toolchain
//! depends on for cross-platform determinism.

use core::f64::consts::PI;

/// Cumulative distribution function of the standard normal `N(0, 1)`.
///
/// Computed as `0.5 * (1 + erf(x / sqrt(2)))`. The accuracy of `libm::erf`
/// (≈ 1 ulp on the relevant range) is more than sufficient for option
/// pricing.
#[inline]
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + libm::erf(x / libm::sqrt(2.0)))
}

/// Probability density function of the standard normal `N(0, 1)`.
///
/// `phi(x) = exp(-x^2 / 2) / sqrt(2 * pi)`.
#[inline]
pub fn norm_pdf(x: f64) -> f64 {
    libm::exp(-0.5 * x * x) / libm::sqrt(2.0 * PI)
}
