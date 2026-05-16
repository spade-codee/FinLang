//! Scope and variable-shadowing tests.
//!
//! Verifies that:
//! * Variables declared in inner blocks shadow outer ones without corrupting
//!   the outer scope after the inner block exits.
//! * The loop variable of a `for` statement is visible only inside the body.
//! * User-defined function parameters are scoped to the function body.
//! * Unknown identifiers produce an `UnknownIdentifier` error.

use finlang_types::{check_str, TypeError};

fn ok(src: &str) {
    let r = check_str(src);
    assert!(
        r.errors.is_empty(),
        "expected no errors for:\n{src}\nbut got: {:#?}",
        r.errors
    );
}

// ── Global / top-level binding ────────────────────────────────────────────────

#[test]
fn global_let_is_accessible_later() {
    ok("let x: price = 100.0 as price\nlet y: price = x");
}

#[test]
fn unknown_identifier_error() {
    let r = check_str("let y: price = unknown_var");
    assert_eq!(r.errors.len(), 1);
    assert!(matches!(r.errors[0], TypeError::UnknownIdentifier { .. }));
}

// ── Shadowing ──────────────────────────────────────────────────────────────────

#[test]
fn shadowing_inner_does_not_leak() {
    // `x` is price at outer level; inside the if-block it shadows as `rate`.
    // After the block, the outer `price` binding should still be there.
    let src = "
let x: price = 100.0 as price
let cond: bool = true
let check: price = if cond {
    let x: rate = 0.05
    x + 0.0
} else {
    x
}
";
    // The inner `x` is rate; else branch sees outer x (price).
    // The two branches have different types → IfBranchMismatch.
    // We only care that the code doesn't panic and the outer x is price.
    let r = check_str(src);
    // There should be a branch mismatch between rate (then) and price (else).
    assert!(
        r.errors
            .iter()
            .any(|e| matches!(e, TypeError::IfBranchMismatch { .. })),
        "expected IfBranchMismatch, got: {:#?}",
        r.errors
    );
}

#[test]
fn top_level_shadowing_ok() {
    // Top-level re-declaration with the same type is fine.
    ok("let x: price = 10.0 as price\nlet x: price = 20.0 as price");
}

// ── For-loop scope ─────────────────────────────────────────────────────────────

#[test]
fn for_loop_variable_scoped_to_body() {
    // `for` is only valid inside a block — wrap in a function.
    let src = "
fn process(prices: [price]) -> price {
    for p in prices {
        let _doubled: price = p + p
    }
    0.0 as price
}
";
    ok(src);
}

#[test]
fn for_loop_variable_not_accessible_after() {
    // The loop variable `p` should not be accessible outside the loop body.
    let src = "
fn process(prices: [price]) -> price {
    for p in prices {
        let _x: price = p
    }
    p
}
";
    let r = check_str(src);
    assert!(
        r.errors.iter().any(|e| matches!(
            e,
            TypeError::UnknownIdentifier { name, .. } if name == "p"
        )),
        "expected UnknownIdentifier for `p` after loop, got: {:#?}",
        r.errors
    );
}

// ── Function scope ─────────────────────────────────────────────────────────────

#[test]
fn fn_params_in_scope_inside_body() {
    let src = "
fn double_price(x: price) -> price {
    x + x
}
";
    ok(src);
}

#[test]
fn fn_params_not_in_scope_outside() {
    let src = "
fn my_fn(x: price) -> price {
    x
}
let _bad: price = x
";
    let r = check_str(src);
    assert!(
        r.errors.iter().any(|e| matches!(
            e,
            TypeError::UnknownIdentifier { name, .. } if name == "x"
        )),
        "expected UnknownIdentifier for `x` outside fn, got: {:#?}",
        r.errors
    );
}

#[test]
fn fn_is_callable_after_definition() {
    let src = "
fn compute(x: price, r: rate) -> price {
    x * r
}
let result: price = compute(100.0 as price, 0.05)
";
    ok(src);
}

// ── Nested block scopes ────────────────────────────────────────────────────────

#[test]
fn nested_blocks_see_outer_scope() {
    let src = "
let outer: rate = 0.05
let cond: bool = true
let _inner: rate = if cond { outer } else { outer }
";
    ok(src);
}

#[test]
fn block_scope_is_independent() {
    // Two sibling blocks; variables defined in one are not in the other.
    let src = "
let cond: bool = true
let a: price = if cond {
    let v: price = 100.0 as price
    v
} else {
    let v: price = 200.0 as price
    v
}
";
    ok(src);
}

// ── If-branch type compatibility ──────────────────────────────────────────────

#[test]
fn if_branches_same_type_ok() {
    let src = "
let cond: bool = true
let px: price = 100.0 as price
let result: price = if cond { px } else { px }
";
    ok(src);
}

#[test]
fn if_branches_mismatch_error() {
    let src = "
let cond: bool = true
let px: price = 100.0 as price
let rt: rate = 0.05
let _bad = if cond { px } else { rt }
";
    let r = check_str(src);
    assert!(
        r.errors
            .iter()
            .any(|e| matches!(e, TypeError::IfBranchMismatch { .. })),
        "expected IfBranchMismatch, got: {:#?}",
        r.errors
    );
}
