//! FinLang dimensional-analysis type checker.
//!
//! This crate is the core of FinLang's safety guarantee: every binary
//! operation between financial dimensions is checked against a static lookup
//! table at compile time, turning "price + rate" into a compile-time error
//! instead of a silent floating-point bug.
//!
//! # Quick start
//!
//! ```rust
//! use finlang_types::{check_str, FinType};
//!
//! // Well-typed: price + price = price
//! let result = check_str("let x: price = 5.0 as price\nlet y: price = 3.0 as price\nx + y");
//! assert!(result.errors.is_empty());
//!
//! // Ill-typed: price + rate is a dimensional error
//! let result = check_str(
//!     "let x: price = 5.0 as price\nlet y: rate = 0.05\nx + y"
//! );
//! assert!(!result.errors.is_empty());
//! ```
//!
//! # Modules
//!
//! * [`ty`] — the [`FinType`] enum and its scalar proxy [`ty::ScalarFinType`].
//! * [`rules`] — the flat `static` dimensional-rules table and [`rules::lookup_rule`].
//! * [`stdlib_sigs`] — stdlib function signatures and [`stdlib_sigs::lookup_stdlib`].
//! * [`error`] — the [`TypeError`] enum and the codespan-based [`render_error`].
//! * [`check`] — the single-pass AST walker, [`TypeCheckResult`], [`check`],
//!   and [`check_str`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod check;
pub mod error;
pub mod rules;
pub mod stdlib_sigs;
pub mod ty;

pub use check::{check, check_str, SymbolTable, TypeCheckResult};
pub use error::{render_error, TypeError};
pub use ty::FinType;

// Re-export BinOpKind so downstream crates don't need to depend on
// finlang-parser directly just to pattern-match on error fields.
pub use finlang_parser::ast::BinOpKind;
