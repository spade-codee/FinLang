//! The [`FinType`] enum — the central currency of the type checker.
//!
//! Every expression in a FinLang program is assigned a `FinType` by the
//! single-pass AST walk in [`crate::check`].  The dimensional analysis rules
//! operate entirely on these variants; compound variants (`List`, `Fn`) only
//! arise at binding or call sites, never inside the static rules table.

use std::fmt;

use finlang_parser::ast::TypeAnnotation;

/// The type of a FinLang expression.
///
/// Financial dimensions (`Price`, `Rate`, …) carry dimensional semantics: the
/// type checker rejects operations that are dimensionally incoherent (e.g.
/// adding a `price` to a `rate`).  Plain scalars (`Bool`, `Int`) and compound
/// types (`List`, `Fn`) have conventional static-typing semantics.
///
/// The two special variants (`Numeric`, `Unknown`) are internal to the type
/// checker:
///
/// * [`FinType::Numeric`] — assigned to unannotated numeric literals.  It
///   dissolves into a concrete dimension via the binary-op rules or an
///   expected-type context.  If it reaches the top level unchanged, the
///   checker emits [`crate::error::TypeError::UnresolvedLiteralType`].
///
/// * [`FinType::Unknown`] — emitted during error recovery so that the checker
///   can continue and collect subsequent errors instead of cascading.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FinType {
    // ── Financial dimensions ──────────────────────────────────────────────────
    /// A monetary amount (stock price, option premium, bond PV, …).
    Price,
    /// A dimensionless ratio / percentage (yield, vol, delta, …).
    Rate,
    /// A face-value quantity (position size, bond face, …).
    Notional,
    /// A calendar date.
    Date,
    /// A time duration measured in years (ACT/365 or similar).
    Years,
    /// 1/100th of a percent — typically converted to `rate` by dividing by
    /// 10 000.
    BasisPoints,

    // ── Plain scalars ─────────────────────────────────────────────────────────
    /// A boolean value (`true` / `false`).
    Bool,
    /// A 64-bit signed integer.
    Int,
    /// The `Call` / `Put` enumeration accepted by the options stdlib.
    OptionType,

    // ── Compound ──────────────────────────────────────────────────────────────
    /// A homogeneous list whose elements all have type `T`.
    List(Box<FinType>),
    /// A function type: `params -> return`.
    Fn(Vec<FinType>, Box<FinType>),

    // ── Inference & recovery ──────────────────────────────────────────────────
    /// A bare numeric literal whose dimension is not yet known.
    ///
    /// Resolved by binary-op rule lookup or expected-type propagation.
    /// Never valid in a final `TypeCheckResult`.
    Numeric,
    /// Produced during error recovery.
    ///
    /// Any operation on `Unknown` yields `Unknown`, preventing error
    /// cascades.
    Unknown,
}

impl fmt::Display for FinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FinType::Price => write!(f, "price"),
            FinType::Rate => write!(f, "rate"),
            FinType::Notional => write!(f, "notional"),
            FinType::Date => write!(f, "date"),
            FinType::Years => write!(f, "years"),
            FinType::BasisPoints => write!(f, "basis_points"),
            FinType::Bool => write!(f, "bool"),
            FinType::Int => write!(f, "int"),
            FinType::OptionType => write!(f, "option_type"),
            FinType::List(inner) => write!(f, "[{inner}]"),
            FinType::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            FinType::Numeric => write!(f, "numeric"),
            FinType::Unknown => write!(f, "unknown"),
        }
    }
}

/// A flat (non-recursive, `Copy`) subset of [`FinType`] covering only the
/// scalar dimensions.
///
/// Used as keys in the static [`crate::rules::DIMENSIONAL_RULES`] table so
/// that the table can live in a `static` without `Box<FinType>` heap
/// allocations or `const` limitations.  Converted to/from [`FinType`] at the
/// lookup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarFinType {
    /// [`FinType::Price`]
    Price,
    /// [`FinType::Rate`]
    Rate,
    /// [`FinType::Notional`]
    Notional,
    /// [`FinType::Date`]
    Date,
    /// [`FinType::Years`]
    Years,
    /// [`FinType::BasisPoints`]
    BasisPoints,
    /// [`FinType::Bool`]
    Bool,
    /// [`FinType::Int`]
    Int,
    /// [`FinType::OptionType`]
    OptionType,
    /// [`FinType::Numeric`]
    Numeric,
    /// [`FinType::Unknown`]
    Unknown,
}

impl ScalarFinType {
    /// Lift to the full [`FinType`].
    #[must_use]
    pub fn to_fin_type(self) -> FinType {
        match self {
            ScalarFinType::Price => FinType::Price,
            ScalarFinType::Rate => FinType::Rate,
            ScalarFinType::Notional => FinType::Notional,
            ScalarFinType::Date => FinType::Date,
            ScalarFinType::Years => FinType::Years,
            ScalarFinType::BasisPoints => FinType::BasisPoints,
            ScalarFinType::Bool => FinType::Bool,
            ScalarFinType::Int => FinType::Int,
            ScalarFinType::OptionType => FinType::OptionType,
            ScalarFinType::Numeric => FinType::Numeric,
            ScalarFinType::Unknown => FinType::Unknown,
        }
    }

    /// Project a [`FinType`] to its scalar key, if it is scalar.
    ///
    /// Returns `None` for compound types (`List`, `Fn`).
    #[must_use]
    pub fn from_fin_type(ty: &FinType) -> Option<Self> {
        match ty {
            FinType::Price => Some(ScalarFinType::Price),
            FinType::Rate => Some(ScalarFinType::Rate),
            FinType::Notional => Some(ScalarFinType::Notional),
            FinType::Date => Some(ScalarFinType::Date),
            FinType::Years => Some(ScalarFinType::Years),
            FinType::BasisPoints => Some(ScalarFinType::BasisPoints),
            FinType::Bool => Some(ScalarFinType::Bool),
            FinType::Int => Some(ScalarFinType::Int),
            FinType::OptionType => Some(ScalarFinType::OptionType),
            FinType::Numeric => Some(ScalarFinType::Numeric),
            FinType::Unknown => Some(ScalarFinType::Unknown),
            FinType::List(_) | FinType::Fn(_, _) => None,
        }
    }
}

/// Convert a parsed [`TypeAnnotation`] to a [`FinType`].
///
/// `Named` variants that do not correspond to a known type produce
/// [`FinType::Unknown`] — the type checker will have already emitted an error
/// for those at the binding site.
#[must_use]
pub fn annotation_to_fin_type(ann: &TypeAnnotation) -> FinType {
    match ann {
        TypeAnnotation::Price => FinType::Price,
        TypeAnnotation::Rate => FinType::Rate,
        TypeAnnotation::Notional => FinType::Notional,
        TypeAnnotation::Date => FinType::Date,
        TypeAnnotation::Years => FinType::Years,
        TypeAnnotation::BasisPoints => FinType::BasisPoints,
        TypeAnnotation::Bool => FinType::Bool,
        TypeAnnotation::Int => FinType::Int,
        TypeAnnotation::OptionType => FinType::OptionType,
        TypeAnnotation::List(inner) => FinType::List(Box::new(annotation_to_fin_type(inner))),
        TypeAnnotation::Named(_) => FinType::Unknown,
    }
}
