//! Dimensional-analysis truth table.
//!
//! The entire set of legal and explicitly-forbidden binary operations is
//! encoded as a flat `static` array of [`DimRule`] rows.  **No nested `match`
//! or `if` chains** appear in the binary-op type-checking code path — the
//! checker calls [`lookup_rule`] and pattern-matches on the result.
//!
//! # Table structure
//!
//! Each row is a `(lhs, op, rhs) -> Result<result_type, custom_message>`:
//!
//! * `Ok(T)`  — the operation is legal; the result has type `T`.
//! * `Err(s)` — the operation is explicitly forbidden with a tailored
//!   diagnostic string `s`.  The outer checker wraps this in a
//!   [`crate::error::TypeError::Dimensional`].
//!
//! Rows that are absent from the table fall through to a generic
//! "unsupported operands" diagnostic (also `TypeError::Dimensional`, but
//! without a `custom_msg`).
//!
//! # Numeric coercion
//!
//! [`crate::ty::ScalarFinType::Numeric`] represents an unannotated numeric
//! literal.  `Numeric op T = T` and `T op Numeric = T` rows are included for
//! every dimension so that e.g. `spot * 100.0` type-checks to `price` without
//! an explicit cast.

use finlang_parser::ast::BinOpKind;

use crate::ty::ScalarFinType;

/// A single row of the dimensional-analysis truth table.
#[derive(Debug, Clone, Copy)]
pub struct DimRule {
    /// Left-hand operand type.
    pub lhs: ScalarFinType,
    /// Binary operator.
    pub op: BinOpKind,
    /// Right-hand operand type.
    pub rhs: ScalarFinType,
    /// `Ok(T)` = legal, result type is `T`.
    /// `Err(msg)` = explicitly forbidden with a custom diagnostic.
    pub result: Result<ScalarFinType, &'static str>,
}

// Helper aliases so the table rows stay readable at 100 columns.
use BinOpKind::{Add, And, Div, Eq, Gt, GtEq, Lt, LtEq, Mod, Mul, NotEq, Or, Sub};
use ScalarFinType::{BasisPoints, Bool, Date, Int, Notional, Numeric, Price, Rate, Unknown, Years};

/// The complete dimensional-analysis truth table.
///
/// Searched linearly by [`lookup_rule`].  Rows are ordered so that explicit
/// `Err` rows come before the corresponding wildcard `Numeric` rows, ensuring
/// that forbidden operations are caught even when one operand is `Numeric`.
pub static DIMENSIONAL_RULES: &[DimRule] = &[
    // ── Add ───────────────────────────────────────────────────────────────────
    // Legal
    DimRule { lhs: Price,    op: Add, rhs: Price,    result: Ok(Price) },
    DimRule { lhs: Rate,     op: Add, rhs: Rate,     result: Ok(Rate) },
    DimRule { lhs: Years,    op: Add, rhs: Years,    result: Ok(Years) },
    DimRule { lhs: Date,     op: Add, rhs: Years,    result: Ok(Date) },
    DimRule { lhs: Notional, op: Add, rhs: Notional, result: Ok(Notional) },
    DimRule { lhs: Int,      op: Add, rhs: Int,      result: Ok(Int) },
    // Forbidden
    DimRule {
        lhs: Price, op: Add, rhs: Rate,
        result: Err("cannot add `price` and `rate`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Rate, op: Add, rhs: Price,
        result: Err("cannot add `rate` and `price`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Price, op: Add, rhs: Notional,
        result: Err("cannot add `price` and `notional`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Notional, op: Add, rhs: Price,
        result: Err("cannot add `notional` and `price`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Rate, op: Add, rhs: Years,
        result: Err("cannot add `rate` and `years`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Years, op: Add, rhs: Rate,
        result: Err("cannot add `years` and `rate`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Date, op: Add, rhs: Date,
        result: Err("adding two dates is dimensionally invalid; subtract them instead"),
    },

    // ── Sub ───────────────────────────────────────────────────────────────────
    // Legal
    DimRule { lhs: Price,    op: Sub, rhs: Price,    result: Ok(Price) },
    DimRule { lhs: Rate,     op: Sub, rhs: Rate,     result: Ok(Rate) },
    DimRule { lhs: Years,    op: Sub, rhs: Years,    result: Ok(Years) },
    DimRule { lhs: Date,     op: Sub, rhs: Date,     result: Ok(Years) }, // elapsed time
    DimRule { lhs: Date,     op: Sub, rhs: Years,    result: Ok(Date) },
    DimRule { lhs: Notional, op: Sub, rhs: Notional, result: Ok(Notional) },
    DimRule { lhs: Int,      op: Sub, rhs: Int,      result: Ok(Int) },
    // Forbidden
    DimRule {
        lhs: Price, op: Sub, rhs: Rate,
        result: Err("cannot subtract `rate` from `price`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Rate, op: Sub, rhs: Price,
        result: Err("cannot subtract `price` from `rate`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Price, op: Sub, rhs: Notional,
        result: Err("cannot subtract `notional` from `price`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Notional, op: Sub, rhs: Price,
        result: Err("cannot subtract `price` from `notional`: incompatible financial dimensions"),
    },
    DimRule {
        lhs: Years, op: Sub, rhs: Date,
        result: Err("cannot subtract `date` from `years`: incompatible financial dimensions"),
    },

    // ── Mul ───────────────────────────────────────────────────────────────────
    // Legal
    DimRule { lhs: Price,    op: Mul, rhs: Rate,     result: Ok(Price) },
    DimRule { lhs: Rate,     op: Mul, rhs: Price,    result: Ok(Price) }, // commutativity
    DimRule { lhs: Notional, op: Mul, rhs: Rate,     result: Ok(Price) },
    DimRule { lhs: Rate,     op: Mul, rhs: Notional, result: Ok(Price) }, // commutativity
    DimRule { lhs: Rate,     op: Mul, rhs: Rate,     result: Ok(Rate) },  // compounding
    DimRule { lhs: Rate,     op: Mul, rhs: Years,    result: Ok(Rate) },
    DimRule { lhs: Years,    op: Mul, rhs: Rate,     result: Ok(Rate) },
    DimRule { lhs: Int,      op: Mul, rhs: Int,      result: Ok(Int) },
    // Forbidden
    DimRule {
        lhs: Price, op: Mul, rhs: Price,
        result: Err("multiplying two prices is dimensionally invalid"),
    },
    DimRule {
        lhs: Notional, op: Mul, rhs: Price,
        result: Err("cannot multiply `notional` by `price`; did you mean `notional * rate`?"),
    },
    DimRule {
        lhs: Price, op: Mul, rhs: Notional,
        result: Err("cannot multiply `price` by `notional`; did you mean `notional * rate`?"),
    },
    DimRule {
        lhs: Notional, op: Mul, rhs: Notional,
        result: Err("multiplying two notionals is dimensionally invalid"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Price,
        result: Err("cannot multiply `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Rate,
        result: Err("cannot multiply `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Years,
        result: Err("cannot multiply `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Notional,
        result: Err("cannot multiply `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Int,
        result: Err("cannot multiply `date` by any dimension"),
    },
    DimRule {
        lhs: Price, op: Mul, rhs: Date,
        result: Err("cannot multiply by `date`"),
    },
    DimRule {
        lhs: Rate, op: Mul, rhs: Date,
        result: Err("cannot multiply by `date`"),
    },
    DimRule {
        lhs: Years, op: Mul, rhs: Date,
        result: Err("cannot multiply by `date`"),
    },
    DimRule {
        lhs: Notional, op: Mul, rhs: Date,
        result: Err("cannot multiply by `date`"),
    },
    DimRule {
        lhs: Int, op: Mul, rhs: Date,
        result: Err("cannot multiply by `date`"),
    },
    DimRule {
        lhs: Date, op: Mul, rhs: Date,
        result: Err("cannot multiply `date` by any dimension"),
    },

    // ── Div ───────────────────────────────────────────────────────────────────
    // Legal
    DimRule { lhs: Price,       op: Div, rhs: Price,    result: Ok(Rate) },   // return
    DimRule { lhs: Price,       op: Div, rhs: Notional, result: Ok(Rate) },
    DimRule { lhs: Notional,    op: Div, rhs: Notional, result: Ok(Rate) },
    DimRule { lhs: BasisPoints, op: Div, rhs: Int,      result: Ok(Rate) },   // unit conversion
    DimRule { lhs: Rate,        op: Div, rhs: Rate,     result: Ok(Rate) },
    DimRule { lhs: Years,       op: Div, rhs: Years,    result: Ok(Rate) },
    DimRule { lhs: Int,         op: Div, rhs: Int,      result: Ok(Int) },
    // Forbidden: Date / anything
    DimRule {
        lhs: Date, op: Div, rhs: Price,
        result: Err("cannot divide `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Div, rhs: Rate,
        result: Err("cannot divide `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Div, rhs: Years,
        result: Err("cannot divide `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Div, rhs: Notional,
        result: Err("cannot divide `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Div, rhs: Int,
        result: Err("cannot divide `date` by any dimension"),
    },
    DimRule {
        lhs: Date, op: Div, rhs: Date,
        result: Err("cannot divide `date` by any dimension"),
    },

    // ── Mod ───────────────────────────────────────────────────────────────────
    DimRule { lhs: Int, op: Mod, rhs: Int, result: Ok(Int) },

    // ── Comparisons: same type → Bool ─────────────────────────────────────────
    DimRule { lhs: Price,    op: Eq,    rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Price,    op: NotEq, rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Price,    op: Lt,    rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Price,    op: Gt,    rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Price,    op: LtEq,  rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Price,    op: GtEq,  rhs: Price,    result: Ok(Bool) },
    DimRule { lhs: Rate,     op: Eq,    rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Rate,     op: NotEq, rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Rate,     op: Lt,    rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Rate,     op: Gt,    rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Rate,     op: LtEq,  rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Rate,     op: GtEq,  rhs: Rate,     result: Ok(Bool) },
    DimRule { lhs: Notional, op: Eq,    rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Notional, op: NotEq, rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Notional, op: Lt,    rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Notional, op: Gt,    rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Notional, op: LtEq,  rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Notional, op: GtEq,  rhs: Notional, result: Ok(Bool) },
    DimRule { lhs: Date,     op: Eq,    rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Date,     op: NotEq, rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Date,     op: Lt,    rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Date,     op: Gt,    rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Date,     op: LtEq,  rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Date,     op: GtEq,  rhs: Date,     result: Ok(Bool) },
    DimRule { lhs: Years,    op: Eq,    rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: Years,    op: NotEq, rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: Years,    op: Lt,    rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: Years,    op: Gt,    rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: Years,    op: LtEq,  rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: Years,    op: GtEq,  rhs: Years,    result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: Eq,    rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: NotEq, rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: Lt,    rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: Gt,    rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: LtEq,  rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: BasisPoints, op: GtEq,  rhs: BasisPoints, result: Ok(Bool) },
    DimRule { lhs: Int,      op: Eq,    rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Int,      op: NotEq, rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Int,      op: Lt,    rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Int,      op: Gt,    rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Int,      op: LtEq,  rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Int,      op: GtEq,  rhs: Int,      result: Ok(Bool) },
    DimRule { lhs: Bool,     op: Eq,    rhs: Bool,     result: Ok(Bool) },
    DimRule { lhs: Bool,     op: NotEq, rhs: Bool,     result: Ok(Bool) },

    // ── Logical ───────────────────────────────────────────────────────────────
    DimRule { lhs: Bool, op: And, rhs: Bool, result: Ok(Bool) },
    DimRule { lhs: Bool, op: Or,  rhs: Bool, result: Ok(Bool) },

    // ── Numeric coercion: Numeric op T = T and T op Numeric = T ──────────────
    //
    // Arithmetic (Add / Sub / Mul / Div / Mod)
    DimRule { lhs: Numeric, op: Add, rhs: Numeric,    result: Ok(Numeric) },
    DimRule { lhs: Numeric, op: Add, rhs: Price,      result: Ok(Price) },
    DimRule { lhs: Price,   op: Add, rhs: Numeric,    result: Ok(Price) },
    DimRule { lhs: Numeric, op: Add, rhs: Rate,       result: Ok(Rate) },
    DimRule { lhs: Rate,    op: Add, rhs: Numeric,    result: Ok(Rate) },
    DimRule { lhs: Numeric, op: Add, rhs: Notional,   result: Ok(Notional) },
    DimRule { lhs: Notional,op: Add, rhs: Numeric,    result: Ok(Notional) },
    DimRule { lhs: Numeric, op: Add, rhs: Years,      result: Ok(Years) },
    DimRule { lhs: Years,   op: Add, rhs: Numeric,    result: Ok(Years) },
    DimRule { lhs: Numeric, op: Add, rhs: BasisPoints,result: Ok(BasisPoints) },
    DimRule { lhs: BasisPoints,op:Add,rhs: Numeric,   result: Ok(BasisPoints) },
    DimRule { lhs: Numeric, op: Add, rhs: Int,        result: Ok(Int) },
    DimRule { lhs: Int,     op: Add, rhs: Numeric,    result: Ok(Int) },

    DimRule { lhs: Numeric, op: Sub, rhs: Numeric,    result: Ok(Numeric) },
    DimRule { lhs: Numeric, op: Sub, rhs: Price,      result: Ok(Price) },
    DimRule { lhs: Price,   op: Sub, rhs: Numeric,    result: Ok(Price) },
    DimRule { lhs: Numeric, op: Sub, rhs: Rate,       result: Ok(Rate) },
    DimRule { lhs: Rate,    op: Sub, rhs: Numeric,    result: Ok(Rate) },
    DimRule { lhs: Numeric, op: Sub, rhs: Notional,   result: Ok(Notional) },
    DimRule { lhs: Notional,op: Sub, rhs: Numeric,    result: Ok(Notional) },
    DimRule { lhs: Numeric, op: Sub, rhs: Years,      result: Ok(Years) },
    DimRule { lhs: Years,   op: Sub, rhs: Numeric,    result: Ok(Years) },
    DimRule { lhs: Numeric, op: Sub, rhs: BasisPoints,result: Ok(BasisPoints) },
    DimRule { lhs: BasisPoints,op:Sub,rhs: Numeric,   result: Ok(BasisPoints) },
    DimRule { lhs: Numeric, op: Sub, rhs: Int,        result: Ok(Int) },
    DimRule { lhs: Int,     op: Sub, rhs: Numeric,    result: Ok(Int) },

    DimRule { lhs: Numeric, op: Mul, rhs: Numeric,    result: Ok(Numeric) },
    DimRule { lhs: Numeric, op: Mul, rhs: Price,      result: Ok(Price) },
    DimRule { lhs: Price,   op: Mul, rhs: Numeric,    result: Ok(Price) },
    DimRule { lhs: Numeric, op: Mul, rhs: Rate,       result: Ok(Rate) },
    DimRule { lhs: Rate,    op: Mul, rhs: Numeric,    result: Ok(Rate) },
    DimRule { lhs: Numeric, op: Mul, rhs: Notional,   result: Ok(Notional) },
    DimRule { lhs: Notional,op: Mul, rhs: Numeric,    result: Ok(Notional) },
    DimRule { lhs: Numeric, op: Mul, rhs: Years,      result: Ok(Years) },
    DimRule { lhs: Years,   op: Mul, rhs: Numeric,    result: Ok(Years) },
    DimRule { lhs: Numeric, op: Mul, rhs: BasisPoints,result: Ok(BasisPoints) },
    DimRule { lhs: BasisPoints,op:Mul,rhs: Numeric,   result: Ok(BasisPoints) },
    DimRule { lhs: Numeric, op: Mul, rhs: Int,        result: Ok(Int) },
    DimRule { lhs: Int,     op: Mul, rhs: Numeric,    result: Ok(Int) },

    DimRule { lhs: Numeric, op: Div, rhs: Numeric,    result: Ok(Numeric) },
    DimRule { lhs: Numeric, op: Div, rhs: Price,      result: Ok(Price) },
    DimRule { lhs: Price,   op: Div, rhs: Numeric,    result: Ok(Price) },
    DimRule { lhs: Numeric, op: Div, rhs: Rate,       result: Ok(Rate) },
    DimRule { lhs: Rate,    op: Div, rhs: Numeric,    result: Ok(Rate) },
    DimRule { lhs: Numeric, op: Div, rhs: Notional,   result: Ok(Notional) },
    DimRule { lhs: Notional,op: Div, rhs: Numeric,    result: Ok(Notional) },
    DimRule { lhs: Numeric, op: Div, rhs: Years,      result: Ok(Years) },
    DimRule { lhs: Years,   op: Div, rhs: Numeric,    result: Ok(Years) },
    DimRule { lhs: Numeric, op: Div, rhs: BasisPoints,result: Ok(BasisPoints) },
    DimRule { lhs: BasisPoints,op:Div,rhs: Numeric,   result: Ok(Rate) }, // bp/numeric ~ rate
    DimRule { lhs: Numeric, op: Div, rhs: Int,        result: Ok(Int) },
    DimRule { lhs: Int,     op: Div, rhs: Numeric,    result: Ok(Int) },

    DimRule { lhs: Numeric, op: Mod, rhs: Numeric, result: Ok(Numeric) },
    DimRule { lhs: Numeric, op: Mod, rhs: Int,     result: Ok(Int) },
    DimRule { lhs: Int,     op: Mod, rhs: Numeric, result: Ok(Int) },

    // Comparisons involving Numeric
    DimRule { lhs: Numeric, op: Eq,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: NotEq, rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Lt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Gt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: LtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: GtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Eq,    rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: Eq,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: NotEq, rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: NotEq, rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Lt,    rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: Lt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Gt,    rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: Gt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: LtEq,  rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: LtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: GtEq,  rhs: Price,      result: Ok(Bool) },
    DimRule { lhs: Price,   op: GtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Eq,    rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: Eq,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: NotEq, rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: NotEq, rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Lt,    rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: Lt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Gt,    rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: Gt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: LtEq,  rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: LtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: GtEq,  rhs: Rate,       result: Ok(Bool) },
    DimRule { lhs: Rate,    op: GtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Eq,    rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: Eq,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: NotEq, rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: NotEq, rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Lt,    rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: Lt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: Gt,    rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: Gt,    rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: LtEq,  rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: LtEq,  rhs: Numeric,    result: Ok(Bool) },
    DimRule { lhs: Numeric, op: GtEq,  rhs: Int,        result: Ok(Bool) },
    DimRule { lhs: Int,     op: GtEq,  rhs: Numeric,    result: Ok(Bool) },

    // Unknown propagation: Unknown op anything = Unknown
    DimRule { lhs: Unknown, op: Add, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Sub, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Mul, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Div, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Mod, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Eq,    rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: NotEq, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Lt,    rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Gt,    rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: LtEq,  rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: GtEq,  rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: And, rhs: Unknown, result: Ok(Unknown) },
    DimRule { lhs: Unknown, op: Or,  rhs: Unknown, result: Ok(Unknown) },
];

/// Look up the dimensional rule for `(lhs, op, rhs)`.
///
/// Returns `Some(&rule)` if a matching row is found, `None` if the
/// combination is absent from the table (which means "unsupported" in the
/// checker).
///
/// # Unknown propagation
///
/// If either operand is `Unknown` the function returns a synthetic
/// `Ok(Unknown)` without consulting the table, preventing error cascades
/// from a single bad sub-expression from flooding diagnostics.
#[must_use]
pub fn lookup_rule(
    lhs: &crate::ty::FinType,
    op: BinOpKind,
    rhs: &crate::ty::FinType,
) -> Option<&'static DimRule> {
    use crate::ty::FinType;

    // Error-recovery: Unknown short-circuits the lookup.
    if *lhs == FinType::Unknown || *rhs == FinType::Unknown {
        // Return the first Unknown-propagation rule to satisfy the borrow.
        // We pick the Add/Unknown row; the result is always Ok(Unknown).
        return DIMENSIONAL_RULES
            .iter()
            .find(|r| r.lhs == Unknown && r.op == op && r.rhs == Unknown)
            .or_else(|| {
                // Fallback: any Unknown row (e.g. op not in table — still
                // propagate Unknown rather than crash).
                DIMENSIONAL_RULES
                    .iter()
                    .find(|r| r.lhs == Unknown && r.rhs == Unknown)
            });
    }

    let lhs_s = crate::ty::ScalarFinType::from_fin_type(lhs)?;
    let rhs_s = crate::ty::ScalarFinType::from_fin_type(rhs)?;

    DIMENSIONAL_RULES
        .iter()
        .find(|r| r.lhs == lhs_s && r.op == op && r.rhs == rhs_s)
}
