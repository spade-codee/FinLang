//! Error types for the IR lowering pass.

use finlang_types::FinType;
use thiserror::Error;

/// An error that occurred while lowering a type-checked AST to IR.
///
/// In a well-typed program, none of these errors should be reachable —
/// they exist as defensive checks against internal inconsistencies or
/// language constructs that the IR intentionally does not support (e.g.
/// `for` loops, list literals, unresolved `Unknown` types).
#[derive(Debug, Error, Clone, PartialEq)]
pub enum LowerError {
    /// The source program contains a construct that the IR lowering does not
    /// support (e.g. `for` loops or list literals).
    #[error("unsupported construct: {0}")]
    UnsupportedConstruct(&'static str),

    /// A FinType that cannot be mapped to an [`crate::ir::IrType`] was
    /// encountered (e.g. `List`, `Fn`, or `Unknown`).
    #[error("type cannot be lowered to IR: {0}")]
    UnsupportedType(FinType),

    /// A variable was referenced that is not in scope.
    ///
    /// This should be impossible after type checking; it indicates an internal
    /// compiler bug.
    #[error("undefined variable in lowering: {0}")]
    UndefinedVariable(String),

    /// The type map from the type checker is missing an entry for an
    /// expression span.  Indicates a type-checker bug.
    #[error("no type information for expression span {0:?}")]
    MissingTypeInfo(finlang_lexer::Span),
}
