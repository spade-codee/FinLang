//! Stdlib function signatures for the FinLang type checker.
//!
//! Every function in `finlang-stdlib` is mirrored here as a [`Signature`].
//! The checker calls [`lookup_stdlib`] during call-expression type checking to
//! validate argument count and dimensions.
//!
//! The signature table is entirely `static` — no heap allocation at startup.
//! Parameters are stored as `&'static [ScalarFinType]` because no stdlib
//! function takes a `List` or `Fn` argument; the conversion to `FinType` at
//! the call site is free (a single match arm per scalar).

use std::sync::OnceLock;

use crate::ty::{FinType, ScalarFinType};

/// The type signature of a stdlib function.
#[derive(Debug)]
pub struct Signature {
    /// Parameter types in declaration order.
    pub params: Vec<FinType>,
    /// Return type.
    pub ret: FinType,
}

/// Registered stdlib signatures.
static REGISTRY: OnceLock<Vec<(&'static str, Signature)>> = OnceLock::new();

/// Build the stdlib signature registry from the static scalar arrays.
fn build_registry() -> Vec<(&'static str, Signature)> {
    // Helper: convert a slice of `ScalarFinType` to `Vec<FinType>`.
    fn params(scalars: &[ScalarFinType]) -> Vec<FinType> {
        scalars.iter().map(|s| s.to_fin_type()).collect()
    }

    use ScalarFinType::{Int, OptionType, Price, Rate, Notional, Years};

    vec![
        // ── Black-Scholes pricing ─────────────────────────────────────────────
        // black_scholes(spot, strike, vol, r, t, opt) -> price
        (
            "black_scholes",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years, OptionType]),
                ret: FinType::Price,
            },
        ),
        // bs_delta(spot, strike, vol, r, t, opt) -> rate
        (
            "bs_delta",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years, OptionType]),
                ret: FinType::Rate,
            },
        ),
        // bs_gamma(spot, strike, vol, r, t) -> rate
        (
            "bs_gamma",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years]),
                ret: FinType::Rate,
            },
        ),
        // bs_vega(spot, strike, vol, r, t) -> price
        (
            "bs_vega",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years]),
                ret: FinType::Price,
            },
        ),
        // bs_theta(spot, strike, vol, r, t, opt) -> price
        (
            "bs_theta",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years, OptionType]),
                ret: FinType::Price,
            },
        ),
        // bs_rho(spot, strike, vol, r, t, opt) -> price
        (
            "bs_rho",
            Signature {
                params: params(&[Price, Price, Rate, Rate, Years, OptionType]),
                ret: FinType::Price,
            },
        ),
        // implied_vol(market_price, spot, strike, r, t, opt) -> rate
        (
            "implied_vol",
            Signature {
                params: params(&[Price, Price, Price, Rate, Years, OptionType]),
                ret: FinType::Rate,
            },
        ),
        // ── Fixed-income ─────────────────────────────────────────────────────
        // bond_price(face, coupon, ytm, periods) -> price
        (
            "bond_price",
            Signature {
                params: params(&[Notional, Rate, Rate, Int]),
                ret: FinType::Price,
            },
        ),
        // bond_duration(face, coupon, ytm, periods) -> years
        (
            "bond_duration",
            Signature {
                params: params(&[Notional, Rate, Rate, Int]),
                ret: FinType::Years,
            },
        ),
        // pv01(face, coupon, ytm, periods) -> price
        (
            "pv01",
            Signature {
                params: params(&[Notional, Rate, Rate, Int]),
                ret: FinType::Price,
            },
        ),
        // ── Rates / forwards ─────────────────────────────────────────────────
        // discount_factor(rate, t) -> rate
        (
            "discount_factor",
            Signature {
                params: params(&[Rate, Years]),
                ret: FinType::Rate,
            },
        ),
        // forward_price(spot, r, t) -> price
        (
            "forward_price",
            Signature {
                params: params(&[Price, Rate, Years]),
                ret: FinType::Price,
            },
        ),
    ]
}

/// Look up the [`Signature`] for a stdlib function by name.
///
/// Returns `None` if the name is not in the stdlib.
///
/// # Examples
///
/// ```rust
/// use finlang_types::stdlib_sigs::lookup_stdlib;
/// use finlang_types::FinType;
///
/// let sig = lookup_stdlib("black_scholes").unwrap();
/// assert_eq!(sig.ret, FinType::Price);
/// assert_eq!(sig.params.len(), 6);
/// ```
#[must_use]
pub fn lookup_stdlib(name: &str) -> Option<&'static Signature> {
    let registry = REGISTRY.get_or_init(build_registry);
    registry
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, sig)| sig)
}

/// Return an iterator over all registered stdlib function names.
///
/// Useful for diagnostics ("did you mean `bond_price`?").
pub fn stdlib_names() -> impl Iterator<Item = &'static str> {
    REGISTRY
        .get_or_init(build_registry)
        .iter()
        .map(|(name, _)| *name)
}
