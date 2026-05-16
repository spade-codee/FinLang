//! Abstract syntax tree types for FinLang.
//!
//! Every node carries a [`Span`] so that diagnostics can point at precise
//! source locations.  All types implement [`Debug`], [`Clone`], and [`PartialEq`]
//! so they are suitable for snapshot testing and for use in hash maps.
//!
//! Conditionally, all types also implement [`serde::Serialize`] and
//! [`serde::Deserialize`] when the `serde` Cargo feature is enabled —
//! required by the `insta` yaml snapshot tests.

use finlang_lexer::Span;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ── Literal kind ──────────────────────────────────────────────────────────────

/// The value held by a literal expression node.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LiteralKind {
    /// A 64-bit signed integer, e.g. `42`.
    Int(i64),
    /// A 64-bit float, e.g. `3.14`.
    Float(f64),
    /// A boolean literal: `true` or `false`.
    Bool(bool),
    /// A string literal with escape sequences already resolved.
    String(String),
    /// The option-type keyword `Call`.
    Call,
    /// The option-type keyword `Put`.
    Put,
}

// ── Binary operator ───────────────────────────────────────────────────────────

/// A binary infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BinOpKind {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    LtEq,
    /// `>=`
    GtEq,
    /// `&&`
    And,
    /// `||`
    Or,
}

// ── Unary operator ────────────────────────────────────────────────────────────

/// A unary prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UnaryOpKind {
    /// Arithmetic negation `-`.
    Neg,
    /// Logical negation `!`.
    Not,
}

// ── Type annotation ───────────────────────────────────────────────────────────

/// A parsed type annotation, as it appears after `:` or `as`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TypeAnnotation {
    /// `price` — a monetary amount.
    Price,
    /// `rate` — a dimensionless ratio / percentage.
    Rate,
    /// `notional` — a face-value quantity.
    Notional,
    /// `date` — a calendar date.
    Date,
    /// `years` — a time duration in years.
    Years,
    /// `basis_points` — 1/100 of a percent.
    BasisPoints,
    /// `bool` — boolean.
    Bool,
    /// `int` — a 64-bit integer.
    Int,
    /// `option_type` — the `Call`/`Put` enumeration type.
    OptionType,
    /// `[ T ]` — a homogeneous list.
    List(Box<TypeAnnotation>),
    /// An unrecognised identifier used as a type.
    ///
    /// Accepted at parse time to keep the door open for user-defined types;
    /// the type checker will reject unknown names.
    Named(String),
}

// ── Function parameter ────────────────────────────────────────────────────────

/// A single function parameter: `name : type`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Param {
    /// The parameter name.
    pub name: String,
    /// The declared type.
    pub ty: TypeAnnotation,
    /// Source span covering `name : type`.
    pub span: Span,
}

// ── Expression ────────────────────────────────────────────────────────────────

/// An expression node.
///
/// Every variant carries its [`Span`] as the last field so that error
/// messages can point at the exact source region.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Expr {
    /// A literal value: integer, float, bool, string, `Call`, or `Put`.
    Literal(LiteralKind, Span),
    /// A bare identifier reference.
    Ident(String, Span),
    /// A binary infix operation.
    BinOp {
        /// The operator.
        op: BinOpKind,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
        /// Span covering the entire expression.
        span: Span,
    },
    /// A unary prefix operation.
    UnaryOp {
        /// The operator.
        op: UnaryOpKind,
        /// The operand.
        expr: Box<Expr>,
        /// Span covering operator and operand.
        span: Span,
    },
    /// A function call: `name(arg, ...)`.
    ///
    /// Note: only a bare identifier can be a callee in this grammar.
    /// Expressions like `f(x)(y)` are not supported; the callee must be
    /// a single name.  This keeps name resolution straightforward and
    /// avoids the need for first-class function types at the parse level.
    Call {
        /// The function name.
        name: String,
        /// Positional arguments, in source order.
        args: Vec<Expr>,
        /// Span from the name to the closing `)`.
        span: Span,
    },
    /// A type cast: `expr as TYPE`.
    Cast {
        /// The expression being cast.
        expr: Box<Expr>,
        /// The target type.
        ty: TypeAnnotation,
        /// Span covering the entire cast expression.
        span: Span,
    },
    /// A conditional expression: `if COND BLOCK (else BLOCK)?`.
    If {
        /// The condition.
        cond: Box<Expr>,
        /// The then-branch block.
        then_branch: Box<Expr>,
        /// The optional else-branch block.
        else_branch: Option<Box<Expr>>,
        /// Span from `if` to the end of the last branch.
        span: Span,
    },
    /// A brace-delimited block: `{ stmt* expr? }`.
    ///
    /// The optional trailing expression is the value of the block.
    Block(Vec<Stmt>, Option<Box<Expr>>, Span),
    /// An index expression: `expr[index]`.
    Index {
        /// The collection being indexed.
        expr: Box<Expr>,
        /// The index expression.
        index: Box<Expr>,
        /// Span from the start of `expr` to the closing `]`.
        span: Span,
    },
    /// A list literal: `[e0, e1, ...]`.
    List(Vec<Expr>, Span),
}

impl Expr {
    /// Return the source span of this expression node.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, sp)
            | Expr::Ident(_, sp)
            | Expr::Block(_, _, sp)
            | Expr::List(_, sp) => *sp,
            Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::Cast { span, .. }
            | Expr::If { span, .. }
            | Expr::Index { span, .. } => *span,
        }
    }

    /// Return a copy of this expression node with its span replaced.
    ///
    /// Used by the parser to preserve outer-parenthesis spans.
    #[must_use]
    pub fn with_span(self, new_span: Span) -> Expr {
        match self {
            Expr::Literal(k, _) => Expr::Literal(k, new_span),
            Expr::Ident(n, _) => Expr::Ident(n, new_span),
            Expr::BinOp { op, lhs, rhs, .. } => Expr::BinOp { op, lhs, rhs, span: new_span },
            Expr::UnaryOp { op, expr, .. } => Expr::UnaryOp { op, expr, span: new_span },
            Expr::Call { name, args, .. } => Expr::Call { name, args, span: new_span },
            Expr::Cast { expr, ty, .. } => Expr::Cast { expr, ty, span: new_span },
            Expr::If { cond, then_branch, else_branch, .. } => {
                Expr::If { cond, then_branch, else_branch, span: new_span }
            }
            Expr::Block(stmts, tail, _) => Expr::Block(stmts, tail, new_span),
            Expr::Index { expr, index, .. } => Expr::Index { expr, index, span: new_span },
            Expr::List(elems, _) => Expr::List(elems, new_span),
        }
    }
}

// ── Statement ─────────────────────────────────────────────────────────────────

/// A statement inside a block.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Stmt {
    /// `let NAME (: TYPE)? = EXPR`
    Let {
        /// The bound name.
        name: String,
        /// Optional type annotation.
        ty: Option<TypeAnnotation>,
        /// The initialiser expression.
        value: Box<Expr>,
        /// Source span.
        span: Span,
    },
    /// An expression used as a statement (value discarded).
    Expr(Box<Expr>, Span),
    /// `return EXPR?`
    Return(Option<Box<Expr>>, Span),
    /// `for VAR in EXPR BLOCK`
    For {
        /// The loop variable.
        var: String,
        /// The iterable expression.
        iter: Box<Expr>,
        /// The loop body block.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },
}

// ── Portfolio ─────────────────────────────────────────────────────────────────

/// The direction of a portfolio leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LegDirection {
    /// Buy / receive.
    Long,
    /// Sell / pay.
    Short,
}

/// A single leg inside a `portfolio` block.
///
/// ```text
/// long  1000.0 as notional  calls  at strike = 100.0 as price
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PortfolioLeg {
    /// Whether this leg is long or short.
    pub direction: LegDirection,
    /// The size expression (typically a cast: `1000.0 as notional`).
    pub size: Expr,
    /// The instrument name (`calls`, `puts`, `underlying`, …).
    pub instrument: String,
    /// `at NAME = EXPR` clauses in source order.
    ///
    /// When the source writes `at NAME` without `= EXPR`, the value is
    /// synthesised as `Ident(NAME)` so the clause list is always uniform.
    pub at_clauses: Vec<(String, Expr)>,
    /// Source span of the full leg.
    pub span: Span,
}

// ── Top-level item ────────────────────────────────────────────────────────────

/// A top-level declaration or expression in a FinLang source file.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Item {
    /// `fn NAME ( PARAMS ) -> TYPE BLOCK`
    FnDef {
        /// Function name.
        name: String,
        /// Parameter list.
        params: Vec<Param>,
        /// Declared return type.
        return_ty: TypeAnnotation,
        /// Body block expression.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },
    /// `portfolio NAME { LEGS }`
    PortfolioDef {
        /// Portfolio name.
        name: String,
        /// The legs, in source order.
        legs: Vec<PortfolioLeg>,
        /// Source span.
        span: Span,
    },
    /// A top-level `let` declaration.
    LetDecl {
        /// The bound name.
        name: String,
        /// Optional type annotation.
        ty: Option<TypeAnnotation>,
        /// The initialiser expression.
        value: Box<Expr>,
        /// Source span.
        span: Span,
    },
    /// A bare top-level expression (e.g. the final `call_price` in
    /// `option_pricing.fin` that the REPL/runner prints).
    ExprItem(Box<Expr>, Span),
}
