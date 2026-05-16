//! Hover handler — given a cursor offset, find the tightest expression span
//! that contains it and report its inferred type.
//!
//! Strategy: the analyzer hands us a `HashMap<Span, FinType>`.  We pick the
//! span that *contains* the cursor and has the **smallest width**, which is
//! the leaf-most expression at that position.

use finlang_lexer::Span;
use finlang_types::FinType;
use ropey::Rope;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::analyze::Analysis;
use crate::convert::span_to_range;

/// Resolve a byte-offset cursor into a hover response.
///
/// Returns [`None`] when no expression span covers the cursor.
#[must_use]
pub fn hover_at(analysis: &Analysis, rope: &Rope, byte_offset: usize) -> Option<Hover> {
    let (span, ty) = tightest_span_at(&analysis.expr_types, byte_offset)?;
    let value = format!("```finlang\n{ty}\n```");
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(span_to_range(rope, span)),
    })
}

/// Find the smallest span in `expr_types` that contains `byte_offset`.
fn tightest_span_at(
    expr_types: &std::collections::HashMap<Span, FinType>,
    byte_offset: usize,
) -> Option<(Span, &FinType)> {
    let mut best: Option<(Span, &FinType)> = None;
    for (span, ty) in expr_types {
        if span.start <= byte_offset && byte_offset <= span.end {
            let width = span.end - span.start;
            match best {
                None => best = Some((*span, ty)),
                Some((cur, _)) if width < (cur.end - cur.start) => best = Some((*span, ty)),
                _ => {}
            }
        }
    }
    best
}
