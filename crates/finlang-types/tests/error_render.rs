//! Snapshot tests for codespan-rendered error messages.
//!
//! Each test type-checks a snippet that produces a known error, then renders
//! it with [`finlang_types::render_error`] and compares the output against an
//! `insta` snapshot.

use finlang_types::{check_str, render_error};

fn render_first(src: &str) -> String {
    let result = check_str(src);
    assert!(
        !result.errors.is_empty(),
        "expected at least one error for:\n{src}"
    );
    render_error("source.fin", src, &result.errors[0])
}

// ── Price + Rate ───────────────────────────────────────────────────────────────

#[test]
fn render_price_plus_rate() {
    let src = "let spot: price = 100.0 as price\nlet vol: rate = 0.20\nspot + vol";
    let rendered = render_first(src);
    // Confirm the key substrings are present.
    assert!(rendered.contains("E001"), "missing error code: {rendered}");
    assert!(
        rendered.contains("price") && rendered.contains("rate"),
        "missing dimension names: {rendered}"
    );
    insta::assert_snapshot!("render_price_plus_rate", rendered);
}

// ── Multiply two prices ────────────────────────────────────────────────────────

#[test]
fn render_mul_two_prices() {
    let src = "let p1: price = 100.0 as price\nlet p2: price = 50.0 as price\np1 * p2";
    let rendered = render_first(src);
    assert!(rendered.contains("E001"), "missing error code: {rendered}");
    insta::assert_snapshot!("render_mul_two_prices", rendered);
}

// ── Wrong stdlib argument type ─────────────────────────────────────────────────

#[test]
fn render_wrong_stdlib_arg() {
    // Pass rate where price is expected (arg 0 of bond_price).
    let src = "let r: rate = 0.05\nlet n: int = 10\nbond_price(r, r, r, n)";
    let rendered = render_first(src);
    assert!(rendered.contains("E002"), "missing error code: {rendered}");
    insta::assert_snapshot!("render_wrong_stdlib_arg", rendered);
}

// ── Adding two dates ───────────────────────────────────────────────────────────

#[test]
fn render_add_two_dates() {
    let src = "let d1: date = 20240101 as date\nlet d2: date = 20240201 as date\nd1 + d2";
    let rendered = render_first(src);
    assert!(rendered.contains("E001"), "missing error code: {rendered}");
    // Custom message for this specific error.
    assert!(
        rendered.contains("subtract"),
        "expected 'subtract' hint: {rendered}"
    );
    insta::assert_snapshot!("render_add_two_dates", rendered);
}

// ── Wrong arity ────────────────────────────────────────────────────────────────

#[test]
fn render_wrong_arity() {
    let src = "let p: price = 100.0 as price\nlet r: rate = 0.05\nblack_scholes(p, r)";
    let rendered = render_first(src);
    assert!(rendered.contains("E003"), "missing error code: {rendered}");
    insta::assert_snapshot!("render_wrong_arity", rendered);
}
