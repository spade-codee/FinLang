//! Goto-definition handler.
//!
//! Given a cursor byte offset, find the identifier under the cursor by
//! tokenising the source string at that location, then search top-level
//! [`Item`]s for a matching `let` or `fn` binding.  Returns the binding's
//! source [`Span`] so the caller can convert it to an LSP `Location`.

use finlang_lexer::{tokenize, Span, Token};
use finlang_parser::ast::Item;

use crate::analyze::Analysis;

/// Resolve a goto-definition request to a source [`Span`].
///
/// Returns `None` when:
/// * The cursor is not on an identifier.
/// * The identifier doesn't match any top-level `let` / `fn` / `portfolio`
///   declaration in the file.
#[must_use]
pub fn definition_at(analysis: &Analysis, source: &str, byte_offset: usize) -> Option<Span> {
    let ident = identifier_at(source, byte_offset)?;
    for item in &analysis.items {
        if let Some((name, span)) = item_binding(item) {
            if name == ident {
                return Some(span);
            }
        }
    }
    None
}

/// Find the identifier token at `byte_offset` (if any) and return its text.
fn identifier_at(source: &str, byte_offset: usize) -> Option<String> {
    let tokens = tokenize(source);
    for spanned in tokens {
        // A cursor sitting at the end byte (one past) should still count.
        let span = spanned.span;
        if span.start <= byte_offset && byte_offset <= span.end {
            if let Token::Ident(name) = spanned.node {
                return Some(name);
            }
        }
    }
    None
}

/// Extract `(name, definition_span)` from a top-level item.
fn item_binding(item: &Item) -> Option<(&str, Span)> {
    match item {
        Item::LetDecl { name, span, .. }
        | Item::FnDef { name, span, .. }
        | Item::PortfolioDef { name, span, .. } => Some((name.as_str(), *span)),
        Item::ExprItem(_, _) => None,
    }
}
