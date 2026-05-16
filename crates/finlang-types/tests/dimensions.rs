//! Dimensional-analysis rule tests.
//!
//! Every legal and forbidden binary-operation rule is exercised here.
//! Legal rules are checked with `assert!(errors.is_empty())`.
//! Forbidden rules are checked with `assert_eq!(errors.len(), 1)` and then
//! pattern-matched to confirm the operand types and operator.

use finlang_types::{check_str, BinOpKind, FinType, TypeError};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ok(src: &str) {
    let r = check_str(src);
    assert!(
        r.errors.is_empty(),
        "expected no errors for `{src}` but got: {:#?}",
        r.errors
    );
}

fn one_dimensional(src: &str, expected_lhs: FinType, expected_op: BinOpKind, expected_rhs: FinType) {
    let r = check_str(src);
    assert_eq!(
        r.errors.len(),
        1,
        "expected exactly 1 error for `{src}` but got {}: {:#?}",
        r.errors.len(),
        r.errors
    );
    match &r.errors[0] {
        TypeError::Dimensional { lhs, op, rhs, .. } => {
            assert_eq!(*lhs, expected_lhs, "lhs mismatch in `{src}`");
            assert_eq!(*op, expected_op, "op mismatch in `{src}`");
            assert_eq!(*rhs, expected_rhs, "rhs mismatch in `{src}`");
        }
        other => panic!("expected Dimensional error for `{src}`, got {other:?}"),
    }
}

// ── Setup snippet that defines typed variables ─────────────────────────────────

const SETUP: &str = "
let px: price = 10.0 as price
let px2: price = 20.0 as price
let rt: rate = 0.05
let rt2: rate = 0.10
let nt: notional = 1000.0 as notional
let nt2: notional = 2000.0 as notional
let dt: date = 20240101 as date
let dt2: date = 20240201 as date
let yr: years = 0.5
let yr2: years = 1.0
let bp: basis_points = 5.0 as basis_points
let n: int = 10
let n2: int = 3
let b1: bool = true
let b2: bool = false
";

fn with_setup(expr: &str) -> String {
    format!("{SETUP}\n{expr}")
}

// ── Add — legal ────────────────────────────────────────────────────────────────

#[test]
fn add_price_price() {
    ok(&with_setup("px + px2"));
}

#[test]
fn add_rate_rate() {
    ok(&with_setup("rt + rt2"));
}

#[test]
fn add_years_years() {
    ok(&with_setup("yr + yr2"));
}

#[test]
fn add_date_years() {
    ok(&with_setup("dt + yr"));
}

#[test]
fn add_notional_notional() {
    ok(&with_setup("nt + nt2"));
}

#[test]
fn add_int_int() {
    ok(&with_setup("n + n2"));
}

// ── Add — forbidden ────────────────────────────────────────────────────────────

#[test]
fn add_price_rate_forbidden() {
    one_dimensional(
        &with_setup("px + rt"),
        FinType::Price,
        BinOpKind::Add,
        FinType::Rate,
    );
}

#[test]
fn add_price_notional_forbidden() {
    one_dimensional(
        &with_setup("px + nt"),
        FinType::Price,
        BinOpKind::Add,
        FinType::Notional,
    );
}

#[test]
fn add_rate_years_forbidden() {
    one_dimensional(
        &with_setup("rt + yr"),
        FinType::Rate,
        BinOpKind::Add,
        FinType::Years,
    );
}

#[test]
fn add_date_date_forbidden() {
    one_dimensional(
        &with_setup("dt + dt2"),
        FinType::Date,
        BinOpKind::Add,
        FinType::Date,
    );
}

// ── Sub — legal ────────────────────────────────────────────────────────────────

#[test]
fn sub_price_price() {
    ok(&with_setup("px - px2"));
}

#[test]
fn sub_rate_rate() {
    ok(&with_setup("rt - rt2"));
}

#[test]
fn sub_years_years() {
    ok(&with_setup("yr - yr2"));
}

#[test]
fn sub_date_date_gives_years() {
    ok(&with_setup("dt - dt2"));
}

#[test]
fn sub_date_years() {
    ok(&with_setup("dt - yr"));
}

#[test]
fn sub_notional_notional() {
    ok(&with_setup("nt - nt2"));
}

#[test]
fn sub_int_int() {
    ok(&with_setup("n - n2"));
}

// ── Sub — forbidden ────────────────────────────────────────────────────────────

#[test]
fn sub_price_rate_forbidden() {
    one_dimensional(
        &with_setup("px - rt"),
        FinType::Price,
        BinOpKind::Sub,
        FinType::Rate,
    );
}

#[test]
fn sub_price_notional_forbidden() {
    one_dimensional(
        &with_setup("px - nt"),
        FinType::Price,
        BinOpKind::Sub,
        FinType::Notional,
    );
}

#[test]
fn sub_years_date_forbidden() {
    one_dimensional(
        &with_setup("yr - dt"),
        FinType::Years,
        BinOpKind::Sub,
        FinType::Date,
    );
}

// ── Mul — legal ────────────────────────────────────────────────────────────────

#[test]
fn mul_price_rate() {
    ok(&with_setup("px * rt"));
}

#[test]
fn mul_rate_price_commutative() {
    ok(&with_setup("rt * px"));
}

#[test]
fn mul_notional_rate() {
    ok(&with_setup("nt * rt"));
}

#[test]
fn mul_rate_notional_commutative() {
    ok(&with_setup("rt * nt"));
}

#[test]
fn mul_rate_rate_compounding() {
    ok(&with_setup("rt * rt2"));
}

#[test]
fn mul_rate_years() {
    ok(&with_setup("rt * yr"));
}

#[test]
fn mul_int_int() {
    ok(&with_setup("n * n2"));
}

// ── Mul — forbidden ────────────────────────────────────────────────────────────

#[test]
fn mul_price_price_forbidden() {
    one_dimensional(
        &with_setup("px * px2"),
        FinType::Price,
        BinOpKind::Mul,
        FinType::Price,
    );
}

#[test]
fn mul_notional_price_forbidden() {
    one_dimensional(
        &with_setup("nt * px"),
        FinType::Notional,
        BinOpKind::Mul,
        FinType::Price,
    );
}

#[test]
fn mul_notional_notional_forbidden() {
    one_dimensional(
        &with_setup("nt * nt2"),
        FinType::Notional,
        BinOpKind::Mul,
        FinType::Notional,
    );
}

// ── Div — legal ────────────────────────────────────────────────────────────────

#[test]
fn div_price_price_gives_rate() {
    ok(&with_setup("px / px2"));
}

#[test]
fn div_price_notional_gives_rate() {
    ok(&with_setup("px / nt"));
}

#[test]
fn div_notional_notional_gives_rate() {
    ok(&with_setup("nt / nt2"));
}

#[test]
fn div_basis_points_int_gives_rate() {
    ok(&with_setup("bp / n"));
}

#[test]
fn div_rate_rate() {
    ok(&with_setup("rt / rt2"));
}

#[test]
fn div_years_years() {
    ok(&with_setup("yr / yr2"));
}

#[test]
fn div_int_int() {
    ok(&with_setup("n / n2"));
}

// ── Div — forbidden ────────────────────────────────────────────────────────────

#[test]
fn div_date_anything_forbidden() {
    one_dimensional(
        &with_setup("dt / n"),
        FinType::Date,
        BinOpKind::Div,
        FinType::Int,
    );
}

// ── Mod ────────────────────────────────────────────────────────────────────────

#[test]
fn mod_int_int() {
    ok(&with_setup("n % n2"));
}

// ── Comparisons ────────────────────────────────────────────────────────────────

#[test]
fn eq_price_price() {
    ok(&with_setup("px == px2"));
}

#[test]
fn lt_rate_rate() {
    ok(&with_setup("rt < rt2"));
}

#[test]
fn gt_years_years() {
    ok(&with_setup("yr > yr2"));
}

#[test]
fn lteq_notional_notional() {
    ok(&with_setup("nt <= nt2"));
}

#[test]
fn gteq_int_int() {
    ok(&with_setup("n >= n2"));
}

#[test]
fn eq_bool_bool() {
    ok(&with_setup("b1 == b2"));
}

// ── Logical ────────────────────────────────────────────────────────────────────

#[test]
fn and_bool_bool() {
    ok(&with_setup("b1 && b2"));
}

#[test]
fn or_bool_bool() {
    ok(&with_setup("b1 || b2"));
}

// ── Numeric coercion ──────────────────────────────────────────────────────────

#[test]
fn numeric_plus_price() {
    ok(&with_setup("100.0 + px"));
}

#[test]
fn price_times_numeric() {
    ok(&with_setup("px * 2.0"));
}

#[test]
fn basis_points_div_numeric_gives_rate() {
    ok("let bp: basis_points = 5.0 as basis_points\nlet r: rate = bp / 10000.0");
}

// ── Unresolved literal at top level ───────────────────────────────────────────

#[test]
fn unresolved_literal_error() {
    let r = check_str("let x = 5.0");
    // Should produce an UnresolvedLiteralType error.
    assert!(
        r.errors
            .iter()
            .any(|e| matches!(e, TypeError::UnresolvedLiteralType { .. })),
        "expected UnresolvedLiteralType, got: {:#?}",
        r.errors
    );
}
