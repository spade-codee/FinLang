//! FinLang SSA intermediate representation and optimisation passes.
//!
//! This crate sits between the type checker (`finlang-types`) and code
//! generation (`finlang-codegen`). It provides:
//!
//! * A compact SSA-form [`IrProgram`] / [`IrFunction`] / [`BasicBlock`] /
//!   [`Inst`] data model.
//! * A lowering pass ([`lower`]) that translates a fully type-checked AST into
//!   IR.
//! * Two optimisation passes: [`const_fold`] (constant folding to a fixed point)
//!   and [`dce`] (dead-code elimination).
//! * A [`validate_ssa`] checker that asserts structural invariants on the IR.
//! * A pretty-printer ([`print`] module) implementing [`std::fmt::Display`] for
//!   snapshot-stable textual IR output.
//!
//! # Example
//!
//! ```no_run
//! use finlang_ir::{lower, const_fold, dce, validate_ssa};
//! use finlang_types::check;
//! use finlang_parser::parse_str;
//!
//! let parsed = parse_str("let x: price = 1.0 as price\nx");
//! let types  = check(&parsed.items);
//! assert!(types.errors.is_empty());
//!
//! let mut prog = lower(&parsed.items, &types).unwrap();
//! const_fold(&mut prog);
//! dce(&mut prog);
//! validate_ssa(&prog).unwrap();
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod ir;
pub mod lower;
pub mod opt;
pub mod print;
pub mod validate;

pub use error::LowerError;
pub use ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};
pub use lower::lower;
pub use opt::{const_fold, dce};
pub use validate::validate_ssa;
