//! Cast (`expr as TYPE`) type-checking tests.

use finlang_types::{check_str, FinType, TypeError};

fn ok(src: &str) {
    let r = check_str(src);
    assert!(
        r.errors.is_empty(),
        "expected no errors for:\n{src}\nbut got: {:#?}",
        r.errors
    );
}

fn one_invalid_cast(src: &str, expected_from: FinType, expected_to: FinType) {
    let r = check_str(src);
    assert!(
        !r.errors.is_empty(),
        "expected at least one error for:\n{src}\nbut got none"
    );
    assert!(
        r.errors.iter().any(|e| matches!(
            e,
            TypeError::InvalidCast { from, to, .. }
            if *from == expected_from && *to == expected_to
        )),
        "expected InvalidCast({expected_from}, {expected_to}), got: {:#?}",
        r.errors
    );
}

// ── Numeric literal casts ──────────────────────────────────────────────────────

#[test]
fn numeric_as_price() {
    ok("let p: price = 100.0 as price");
}

#[test]
fn numeric_as_rate() {
    ok("let r: rate = 0.05 as rate");
}

#[test]
fn numeric_as_notional() {
    ok("let n: notional = 1000.0 as notional");
}

#[test]
fn numeric_as_years() {
    ok("let t: years = 0.5 as years");
}

#[test]
fn numeric_as_basis_points() {
    ok("let bp: basis_points = 5.0 as basis_points");
}

#[test]
fn numeric_as_int() {
    ok("let n: int = 10 as int");
}

#[test]
fn int_literal_as_price() {
    ok("let p: price = 100 as price");
}

// ── Basis-points unit conversion ───────────────────────────────────────────────

#[test]
fn basis_points_div_numeric_gives_rate() {
    ok("let bp: basis_points = 1.0 as basis_points\nlet r: rate = bp / 10000.0");
}

#[test]
fn explicit_bp_cast_div() {
    ok("let r: rate = 1.0 as basis_points / 10000.0");
}

// ── Cross-dimension casts — allowed (escape hatch) ────────────────────────────

#[test]
fn price_as_rate_allowed() {
    // `as` is the explicit override; cross-dimension cast must not error.
    ok("let p: price = 100.0 as price\nlet r: rate = p as rate");
}

#[test]
fn rate_as_price_allowed() {
    ok("let r: rate = 0.05\nlet p: price = r as price");
}

#[test]
fn notional_as_price_allowed() {
    ok("let n: notional = 1000.0 as notional\nlet p: price = n as price");
}

#[test]
fn int_as_price_allowed() {
    ok("let n: int = 10\nlet p: price = n as price");
}

#[test]
fn price_as_int_allowed() {
    ok("let p: price = 100.0 as price\nlet n: int = p as int");
}

#[test]
fn numeric_as_date_allowed() {
    ok("let d: date = 20240101 as date");
}

// ── Forbidden casts ────────────────────────────────────────────────────────────

#[test]
fn numeric_as_bool_forbidden() {
    // Numeric → Bool: forbidden.
    // We detect this as Int → Bool or Price → Bool at the cast boundary.
    // The easiest way to force this is to cast an int literal explicitly.
    let r = check_str("let b: bool = 1 as int as bool");
    assert!(
        r.errors
            .iter()
            .any(|e| matches!(e, TypeError::InvalidCast { to, .. } if *to == FinType::Bool)),
        "expected InvalidCast to bool, got: {:#?}",
        r.errors
    );
}

#[test]
fn bool_as_price_forbidden() {
    one_invalid_cast(
        "let b: bool = true\nlet p: price = b as price",
        FinType::Bool,
        FinType::Price,
    );
}

#[test]
fn price_as_option_type_forbidden() {
    one_invalid_cast(
        "let p: price = 100.0 as price\nlet o: option_type = p as option_type",
        FinType::Price,
        FinType::OptionType,
    );
}

#[test]
fn rate_as_option_type_forbidden() {
    one_invalid_cast(
        "let r: rate = 0.05\nlet o: option_type = r as option_type",
        FinType::Rate,
        FinType::OptionType,
    );
}

// ── List casts ─────────────────────────────────────────────────────────────────

#[test]
fn list_price_as_list_rate_allowed() {
    // List(Price) → List(Rate) is allowed (escape hatch).
    ok("let ps: [price] = [10.0 as price, 20.0 as price]\nlet rs: [rate] = ps as [rate]");
}
