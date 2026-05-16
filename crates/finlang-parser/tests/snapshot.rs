//! Insta snapshot tests for the three canonical example files.
//!
//! Uses `assert_debug_snapshot!` (no serde dependency needed) to capture the
//! full AST.  On the first run set `INSTA_UPDATE=always` (or `cargo insta
//! review`) to create the `.snap` files; subsequent runs compare against them.
//!
//! Each test also asserts `errors.is_empty()` — the example files are the
//! parser's conformance contract.

use finlang_parser::{parse_str, ParseResult};

fn read_example(name: &str) -> String {
    // CARGO_MANIFEST_DIR points at `crates/finlang-parser/`
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let path = std::path::Path::new(&manifest)
        .join("../../examples")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn assert_clean(result: &ParseResult, file: &str) {
    assert!(
        result.errors.is_empty(),
        "{file} produced parse errors: {:#?}",
        result.errors
    );
}

#[test]
fn snapshot_option_pricing() {
    let source = read_example("option_pricing.fin");
    let result = parse_str(&source);
    assert_clean(&result, "option_pricing.fin");
    insta::assert_debug_snapshot!("option_pricing_items", result.items);
}

#[test]
fn snapshot_bond_portfolio() {
    let source = read_example("bond_portfolio.fin");
    let result = parse_str(&source);
    assert_clean(&result, "bond_portfolio.fin");
    insta::assert_debug_snapshot!("bond_portfolio_items", result.items);
}

#[test]
fn snapshot_var_calculation() {
    let source = read_example("var_calculation.fin");
    let result = parse_str(&source);
    assert_clean(&result, "var_calculation.fin");
    insta::assert_debug_snapshot!("var_calculation_items", result.items);
}
