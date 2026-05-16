//! FinLang standard library.
//!
//! Hand-written, from-scratch implementations of the financial primitives
//! exposed to FinLang source: Black-Scholes pricing and Greeks, implied
//! volatility (Newton-Raphson), bond pricing, duration, PV01, discount
//! factors, and forward pricing.
//!
//! # Two-tier API
//!
//! Each primitive has two forms:
//!
//! 1. A safe Rust function (e.g. [`black_scholes`]) taking the
//!    [`OptionType`] enum. Use this in Rust callers and unit tests.
//! 2. An `extern "C"` shim (e.g. [`finlang_black_scholes`]) taking `i64` for
//!    the option-type discriminant. Cranelift registers these as external
//!    symbols and calls them from JITed FinLang code with the standard
//!    System V / Win64 C ABI.
//!
//! All `finlang_*` symbols are `#[no_mangle]` and unique-prefixed so they
//! cannot collide with anything in the host process at JIT link time.
//!
//! # Conventions at a glance
//!
//! * **Black-Scholes:** continuously-compounded rates, lognormal vol,
//!   `t` in years.
//! * **Bonds:** annual discrete compounding, `periods` = integer years.
//! * **Greeks:** returned in natural units (per unit vol, per unit time, per
//!   unit rate) — never pre-divided by 100 or 365.
//! * `libm::{exp, log, sqrt, erf, pow, fabs}` are used in place of the
//!   `f64` methods so all numerics are bit-reproducible across hosts.

// `forbid(unsafe_code)` cannot be used at the crate root because modern Rust
// classifies `#[no_mangle]` as an unsafe attribute (it can cause link-time
// symbol collisions). We use `deny` and narrowly `allow` it on the
// extern-ABI module. There are zero `unsafe { ... }` blocks in this crate.
#![deny(unsafe_code)]
#![deny(missing_docs)]

mod abi;
mod bonds;
mod normal;
mod options;
mod rates;

pub use abi::{
    finlang_black_scholes, finlang_bond_duration, finlang_bond_price, finlang_bs_delta,
    finlang_bs_gamma, finlang_bs_rho, finlang_bs_theta, finlang_bs_vega, finlang_discount_factor,
    finlang_forward_price, finlang_implied_vol, finlang_pv01,
};
pub use bonds::{bond_duration, bond_price, pv01};
pub use options::{
    black_scholes, bs_delta, bs_gamma, bs_rho, bs_theta, bs_vega, implied_vol,
};
pub use rates::{discount_factor, forward_price};

/// Type of European option.
///
/// The discriminants are part of the wire-level ABI: FinLang codegen emits
/// `0` for `Call` and `1` for `Put` when calling the `finlang_*` extern
/// functions below.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    /// Right to buy at the strike.
    Call = 0,
    /// Right to sell at the strike.
    Put = 1,
}

impl OptionType {
    /// Convert the wire-level `i64` discriminant to an [`OptionType`].
    ///
    /// Returns `None` for any value other than `0` or `1`; the extern shims
    /// translate that to `NaN`.
    #[inline]
    pub fn from_i64(v: i64) -> Option<Self> {
        match v {
            0 => Some(Self::Call),
            1 => Some(Self::Put),
            _ => None,
        }
    }
}
