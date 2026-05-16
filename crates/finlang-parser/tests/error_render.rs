//! Tests for codespan-reporting diagnostic rendering.
//!
//! The rendered output is first checked for structural substrings
//! (expected/found, source line, span carets) and then snapshotted with
//! `insta` for regression detection.

use finlang_parser::error::render_error;
use finlang_parser::parse_str;

/// Trigger an UnexpectedToken error and validate that the rendered diagnostic
/// contains the canonical substrings a human reader would look for.
#[test]
fn render_unexpected_token_substrings() {
    let source = "let x = ;";
    let result = parse_str(source);
    assert!(
        !result.errors.is_empty(),
        "expected at least one parse error"
    );
    let rendered = render_error("test.fin", source, &result.errors[0]);

    // Must contain the word "expected" (codespan header or label)
    assert!(
        rendered.contains("expected"),
        "rendered output missing 'expected': {rendered}"
    );
    // Must contain the source text somewhere (codespan prints the line)
    assert!(
        rendered.contains("let x = ;"),
        "rendered output missing source line: {rendered}"
    );
    // Must contain caret characters `^` (the span underline)
    assert!(
        rendered.contains('^'),
        "rendered output missing caret underlines: {rendered}"
    );
}

/// Snapshot the full rendered output for reproducible regression testing.
#[test]
fn snapshot_unexpected_token_render() {
    let source = "let x = ;";
    let result = parse_str(source);
    let rendered = render_error("test.fin", source, &result.errors[0]);
    insta::assert_snapshot!(rendered);
}

/// Chained comparison renders with the dedicated message.
#[test]
fn render_chained_comparison() {
    let source = "a < b < c";
    let result = parse_str(source);
    let cmp_err = result
        .errors
        .iter()
        .find(|e| matches!(e, finlang_parser::ParseError::ChainedComparison { .. }))
        .expect("expected ChainedComparison error");
    let rendered = render_error("test.fin", source, cmp_err);
    assert!(
        rendered.contains("chained"),
        "expected 'chained' in rendered output: {rendered}"
    );
}
