//! Integration tests: type-check the three example `.fin` files.
//!
//! All three must produce zero type errors.  The test reads the files from
//! `<CARGO_MANIFEST_DIR>/../../examples/` relative to this crate.

use finlang_types::check_str;
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("examples")
}

fn read_example(name: &str) -> String {
    let path = examples_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn option_pricing_fin_zero_errors() {
    let src = read_example("option_pricing.fin");
    let result = check_str(&src);
    assert!(
        result.errors.is_empty(),
        "option_pricing.fin has type errors: {:#?}",
        result.errors
    );
}

#[test]
fn bond_portfolio_fin_zero_errors() {
    let src = read_example("bond_portfolio.fin");
    let result = check_str(&src);
    assert!(
        result.errors.is_empty(),
        "bond_portfolio.fin has type errors: {:#?}",
        result.errors
    );
}

#[test]
fn var_calculation_fin_zero_errors() {
    let src = read_example("var_calculation.fin");
    let result = check_str(&src);
    assert!(
        result.errors.is_empty(),
        "var_calculation.fin has type errors: {:#?}",
        result.errors
    );
}

// ── Snapshot: the three example files produce no errors ───────────────────────

#[test]
fn option_pricing_errors_snapshot() {
    let src = read_example("option_pricing.fin");
    let result = check_str(&src);
    // The errors list must be empty — this doubles as a snapshot.
    insta::assert_debug_snapshot!("option_pricing_errors", result.errors);
}

#[test]
fn bond_portfolio_errors_snapshot() {
    let src = read_example("bond_portfolio.fin");
    let result = check_str(&src);
    insta::assert_debug_snapshot!("bond_portfolio_errors", result.errors);
}

#[test]
fn var_calculation_errors_snapshot() {
    let src = read_example("var_calculation.fin");
    let result = check_str(&src);
    insta::assert_debug_snapshot!("var_calculation_errors", result.errors);
}

// ── Snapshot: expr_types for option_pricing.fin ───────────────────────────────

#[test]
fn option_pricing_expr_types_snapshot() {
    let src = read_example("option_pricing.fin");
    let result = check_str(&src);
    // Sort by span start for a deterministic snapshot.
    let mut entries: Vec<(usize, String)> = result
        .expr_types
        .iter()
        .map(|(span, ty)| (span.start, format!("{span} => {ty}")))
        .collect();
    entries.sort();
    let rendered: Vec<&str> = entries.iter().map(|(_, s)| s.as_str()).collect();
    insta::assert_debug_snapshot!("option_pricing_expr_types_sorted", rendered);
}
