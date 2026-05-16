//! Snapshot tests for full token streams of the example `.fin` files.
//!
//! On first run (or when `INSTA_UPDATE=always` is set) insta writes
//! `tests/snapshots/*.snap` files.  Subsequent runs compare against those
//! files.  Commit the snapshot files alongside the source.

use finlang_lexer::tokenize;
use std::path::PathBuf;

/// Resolve a path relative to the workspace root.
///
/// `CARGO_MANIFEST_DIR` points to `crates/finlang-lexer`; we go two levels up
/// to reach the workspace root.
fn workspace_path(relative: &str) -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("../..").join(relative)
}

#[test]
fn snapshot_option_pricing() {
    let path = workspace_path("examples/option_pricing.fin");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let tokens = tokenize(&source);
    insta::assert_yaml_snapshot!("option_pricing_tokens", tokens);
}

#[test]
fn snapshot_bond_portfolio() {
    let path = workspace_path("examples/bond_portfolio.fin");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let tokens = tokenize(&source);
    insta::assert_yaml_snapshot!("bond_portfolio_tokens", tokens);
}

#[test]
fn snapshot_var_calculation() {
    let path = workspace_path("examples/var_calculation.fin");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let tokens = tokenize(&source);
    insta::assert_yaml_snapshot!("var_calculation_tokens", tokens);
}
