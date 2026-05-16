//! `extern "C"` ABI shims registered as Cranelift external symbols by
//! `finlang_codegen`.
//!
//! Each function here is `#[no_mangle]` so the JIT can link to it by a fixed
//! symbol name. There are no `unsafe { ... }` blocks; the local
//! `allow(unsafe_code)` is purely to permit the `#[no_mangle]` attribute,
//! which modern Rust (>=1.82) classifies as an unsafe attribute because of
//! link-time symbol-collision risk.

#![allow(unsafe_code)]

use crate::{
    black_scholes, bond_duration, bond_price, bs_delta, bs_gamma, bs_rho, bs_theta, bs_vega,
    discount_factor, forward_price, implied_vol, pv01, OptionType,
};

/// Black-Scholes price (extern ABI). See [`crate::black_scholes`].
#[no_mangle]
pub extern "C" fn finlang_black_scholes(
    spot: f64,
    strike: f64,
    vol: f64,
    r: f64,
    t: f64,
    opt: i64,
) -> f64 {
    match OptionType::from_i64(opt) {
        Some(o) => black_scholes(spot, strike, vol, r, t, o),
        None => f64::NAN,
    }
}

/// Black-Scholes delta (extern ABI). See [`crate::bs_delta`].
#[no_mangle]
pub extern "C" fn finlang_bs_delta(
    spot: f64,
    strike: f64,
    vol: f64,
    r: f64,
    t: f64,
    opt: i64,
) -> f64 {
    match OptionType::from_i64(opt) {
        Some(o) => bs_delta(spot, strike, vol, r, t, o),
        None => f64::NAN,
    }
}

/// Black-Scholes gamma (extern ABI). See [`crate::bs_gamma`].
#[no_mangle]
pub extern "C" fn finlang_bs_gamma(spot: f64, strike: f64, vol: f64, r: f64, t: f64) -> f64 {
    bs_gamma(spot, strike, vol, r, t)
}

/// Black-Scholes vega (extern ABI). See [`crate::bs_vega`].
#[no_mangle]
pub extern "C" fn finlang_bs_vega(spot: f64, strike: f64, vol: f64, r: f64, t: f64) -> f64 {
    bs_vega(spot, strike, vol, r, t)
}

/// Black-Scholes theta (extern ABI). See [`crate::bs_theta`].
#[no_mangle]
pub extern "C" fn finlang_bs_theta(
    spot: f64,
    strike: f64,
    vol: f64,
    r: f64,
    t: f64,
    opt: i64,
) -> f64 {
    match OptionType::from_i64(opt) {
        Some(o) => bs_theta(spot, strike, vol, r, t, o),
        None => f64::NAN,
    }
}

/// Black-Scholes rho (extern ABI). See [`crate::bs_rho`].
#[no_mangle]
pub extern "C" fn finlang_bs_rho(
    spot: f64,
    strike: f64,
    vol: f64,
    r: f64,
    t: f64,
    opt: i64,
) -> f64 {
    match OptionType::from_i64(opt) {
        Some(o) => bs_rho(spot, strike, vol, r, t, o),
        None => f64::NAN,
    }
}

/// Newton-Raphson implied volatility (extern ABI). See [`crate::implied_vol`].
#[no_mangle]
pub extern "C" fn finlang_implied_vol(
    market_price: f64,
    spot: f64,
    strike: f64,
    r: f64,
    t: f64,
    opt: i64,
) -> f64 {
    match OptionType::from_i64(opt) {
        Some(o) => implied_vol(market_price, spot, strike, r, t, o),
        None => f64::NAN,
    }
}

/// Bond price (extern ABI). See [`crate::bond_price`].
#[no_mangle]
pub extern "C" fn finlang_bond_price(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    bond_price(face, coupon, ytm, periods)
}

/// Macaulay duration in years (extern ABI). See [`crate::bond_duration`].
#[no_mangle]
pub extern "C" fn finlang_bond_duration(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    bond_duration(face, coupon, ytm, periods)
}

/// PV01 (extern ABI). See [`crate::pv01`].
#[no_mangle]
pub extern "C" fn finlang_pv01(face: f64, coupon: f64, ytm: f64, periods: i64) -> f64 {
    pv01(face, coupon, ytm, periods)
}

/// Continuous-compounding discount factor (extern ABI). See
/// [`crate::discount_factor`].
#[no_mangle]
pub extern "C" fn finlang_discount_factor(rate: f64, t: f64) -> f64 {
    discount_factor(rate, t)
}

/// Cost-of-carry forward price (extern ABI). See [`crate::forward_price`].
#[no_mangle]
pub extern "C" fn finlang_forward_price(spot: f64, r: f64, t: f64) -> f64 {
    forward_price(spot, r, t)
}
