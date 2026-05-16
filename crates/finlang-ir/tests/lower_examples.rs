//! Integration tests: lower the three example `.fin` files and snapshot the IR.

use finlang_ir::{const_fold, dce, lower, validate_ssa};
use finlang_parser::parse_str;
use finlang_types::check;

fn load_example(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("..")
        .join("..")
        .join("examples")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn lower_source(src: &str) -> finlang_ir::IrProgram {
    let parsed = parse_str(src);
    let types = check(&parsed.items);
    assert!(
        types.errors.is_empty(),
        "type errors in example: {:#?}",
        types.errors
    );
    lower(&parsed.items, &types).expect("lowering failed")
}

// ── option_pricing.fin ────────────────────────────────────────────────────────

#[test]
fn option_pricing_lower_ok() {
    let src = load_example("option_pricing.fin");
    let prog = lower_source(&src);
    assert!(prog.functions.iter().any(|f| f.name == "__main__"));
}

#[test]
fn option_pricing_snapshot_before_opts() {
    let src = load_example("option_pricing.fin");
    let prog = lower_source(&src);
    insta::assert_snapshot!("option_pricing_before", prog.to_string());
}

#[test]
fn option_pricing_snapshot_after_const_fold() {
    let src = load_example("option_pricing.fin");
    let mut prog = lower_source(&src);
    const_fold(&mut prog);
    insta::assert_snapshot!("option_pricing_after_fold", prog.to_string());
}

#[test]
fn option_pricing_snapshot_after_dce() {
    let src = load_example("option_pricing.fin");
    let mut prog = lower_source(&src);
    const_fold(&mut prog);
    dce(&mut prog);
    validate_ssa(&prog).expect("SSA invalid after dce");
    insta::assert_snapshot!("option_pricing_after_dce", prog.to_string());
}

// ── bond_portfolio.fin ────────────────────────────────────────────────────────

#[test]
fn bond_portfolio_lower_ok() {
    let src = load_example("bond_portfolio.fin");
    let prog = lower_source(&src);
    assert!(prog.functions.iter().any(|f| f.name == "__main__"));
}

#[test]
fn bond_portfolio_snapshot_before_opts() {
    let src = load_example("bond_portfolio.fin");
    let prog = lower_source(&src);
    insta::assert_snapshot!("bond_portfolio_before", prog.to_string());
}

#[test]
fn bond_portfolio_snapshot_after_dce() {
    let src = load_example("bond_portfolio.fin");
    let mut prog = lower_source(&src);
    const_fold(&mut prog);
    dce(&mut prog);
    validate_ssa(&prog).expect("SSA invalid after dce");
    insta::assert_snapshot!("bond_portfolio_after_dce", prog.to_string());
}

// ── var_calculation.fin ───────────────────────────────────────────────────────

#[test]
fn var_calculation_lower_ok() {
    let src = load_example("var_calculation.fin");
    let prog = lower_source(&src);
    assert!(prog.functions.iter().any(|f| f.name == "__main__"));
}

#[test]
fn var_calculation_snapshot_before_opts() {
    let src = load_example("var_calculation.fin");
    let prog = lower_source(&src);
    insta::assert_snapshot!("var_calculation_before", prog.to_string());
}

#[test]
fn var_calculation_snapshot_after_dce() {
    let src = load_example("var_calculation.fin");
    let mut prog = lower_source(&src);
    const_fold(&mut prog);
    dce(&mut prog);
    validate_ssa(&prog).expect("SSA invalid after dce");
    insta::assert_snapshot!("var_calculation_after_dce", prog.to_string());
}
