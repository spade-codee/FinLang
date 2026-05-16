//! Front-end analysis used by every LSP handler.
//!
//! [`analyze`] runs the same lex → parse → typecheck pipeline as the CLI but
//! never returns an error — it collects every parse error and type error so
//! the LSP can publish them as diagnostics, and returns the partial AST and
//! the expression-type map so hover/completion/definition can use whatever
//! the parser managed to recover.

use std::collections::HashMap;

use finlang_lexer::Span;
use finlang_parser::{ast::Item, parse_str, ParseError};
use finlang_types::{check, FinType, TypeError};

/// The accumulated result of running the front-end on a single document.
#[derive(Debug, Default)]
pub struct Analysis {
    /// Every parse error in source order.
    pub parse_errors: Vec<ParseError>,
    /// Every type error in source order.
    pub type_errors: Vec<TypeError>,
    /// Top-level items the parser recovered (may be partial).
    pub items: Vec<Item>,
    /// Inferred type of every AST expression node, keyed by span.
    pub expr_types: HashMap<Span, FinType>,
}

/// Run the front-end pipeline on `source` and collect diagnostics.
///
/// Even when parse errors are present, we still attempt type checking on
/// whatever items the parser produced — that gives partial completion /
/// hover information while the user is mid-edit.
#[must_use]
pub fn analyze(source: &str) -> Analysis {
    let parsed = parse_str(source);
    let types = check(&parsed.items);
    Analysis {
        parse_errors: parsed.errors,
        type_errors: types.errors,
        items: parsed.items,
        expr_types: types.expr_types,
    }
}
