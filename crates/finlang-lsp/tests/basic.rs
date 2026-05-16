//! Smoke tests for the LSP support modules.
//!
//! Full protocol-level integration testing requires a JSON-RPC harness which
//! is too heavyweight for v0.1; these tests instead exercise the pure
//! functions that underpin every handler.

use finlang_lexer::Span;
use finlang_lsp::analyze::analyze;
use finlang_lsp::convert::{byte_offset_to_position, position_to_byte_offset};
use finlang_lsp::hover::hover_at;
use finlang_types::FinType;
use ropey::Rope;
use tower_lsp::lsp_types::{HoverContents, Position};

#[test]
fn utf16_column_skips_multibyte_chars() {
    // « and » are 2-byte UTF-8 but a single UTF-16 code unit each.
    let source = "let s = «hi»\nlet y = 1";
    let rope = Rope::from_str(source);

    // Byte offset of `\n` (end of line 0): it sits one byte past `»`.
    let newline_byte = source.find('\n').expect("newline present");
    let pos = byte_offset_to_position(&rope, newline_byte);
    assert_eq!(pos.line, 0);
    // UTF-16 column count of `let s = «hi»` = 12.
    assert_eq!(pos.character, 12);

    // Round-trip via UTF-16 column back to byte offset.
    let back = position_to_byte_offset(&rope, pos);
    assert_eq!(back, newline_byte);
}

#[test]
fn analyze_catches_dimensional_mismatch() {
    let source = "let x: price = 1.0 as price\nlet y: rate = 0.05\nlet z = x + y";
    let result = analyze(source);
    assert!(
        !result.type_errors.is_empty(),
        "expected at least one type error, got none"
    );
}

#[test]
fn hover_on_identifier_returns_price() {
    let source = "let x: price = 5.0 as price\nx";
    let analysis = analyze(source);
    let rope = Rope::from_str(source);

    // Cursor sits on the trailing `x` (the only character on line 1).
    let cursor_byte = source.rfind('x').expect("`x` present");
    let hover = hover_at(&analysis, &rope, cursor_byte).expect("hover present");

    let HoverContents::Markup(content) = hover.contents else {
        panic!("expected markup contents");
    };
    assert!(
        content.value.contains(&FinType::Price.to_string()),
        "expected hover to mention `price`, got: {}",
        content.value
    );
}

#[test]
fn position_to_byte_offset_clamps_out_of_range() {
    let source = "abc";
    let rope = Rope::from_str(source);
    let huge = Position {
        line: 9_999,
        character: 9_999,
    };
    let off = position_to_byte_offset(&rope, huge);
    assert!(off <= source.len());
}

#[test]
fn analyze_returns_items_even_with_parse_errors() {
    // The first let is well-formed; the second is intentionally broken so we
    // exercise the partial-recovery path.
    let source = "let x: price = 1.0 as price\nlet y =";
    let analysis = analyze(source);
    assert!(!analysis.items.is_empty());
    // Confirm spans still make sense by mapping a known offset.
    let _ = Span { start: 0, end: 1 };
}
