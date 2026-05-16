//! Error-recovery tests.
//!
//! Each test feeds the parser a source with multiple syntactic errors and
//! asserts that *all* errors are collected (i.e. the parser does not abort
//! after the first failure).

use finlang_parser::parse_str;

/// `let x = ; let y = 5`
///
/// The first `let` is missing its initialiser expression.  Recovery should
/// resync at the next `let` keyword and parse `y` successfully.
#[test]
fn two_lets_first_malformed() {
    let result = parse_str("let x = ; let y = 5");
    // At least one error for the missing expression in `let x = ;`
    assert!(
        !result.errors.is_empty(),
        "expected at least one error, got none"
    );
    // `y` should still appear in items
    let has_y = result.items.iter().any(|item| {
        matches!(item, finlang_parser::ast::Item::LetDecl { name, .. } if name == "y")
    });
    assert!(has_y, "expected `let y = 5` to be recovered; items = {:?}", result.items);
}

/// `let x = 5 + ; let y = 10 + ; let z = 15`
///
/// Two malformed lets followed by one good one.  The parser must report at
/// least two errors and still parse `z`.
#[test]
fn three_lets_two_malformed() {
    let result = parse_str("let x = 5 + ; let y = 10 + ; let z = 15");
    assert!(
        result.errors.len() >= 2,
        "expected at least 2 errors, got {}: {:?}",
        result.errors.len(),
        result.errors
    );
    let has_z = result.items.iter().any(|item| {
        matches!(item, finlang_parser::ast::Item::LetDecl { name, .. } if name == "z")
    });
    assert!(has_z, "expected `let z = 15` to be recovered; items = {:?}", result.items);
}

/// An empty token stream (just EOF) produces no items and no errors.
#[test]
fn empty_source() {
    let result = parse_str("");
    assert!(result.errors.is_empty(), "empty source should have no errors");
    assert!(result.items.is_empty(), "empty source should have no items");
}

/// A stream containing only `Token::LexError` values must not panic.
#[test]
fn only_lex_errors_no_panic() {
    // `@@@` produces lex errors only
    let result = parse_str("@@@ @@@ @@@");
    // the parser must not panic; errors may be LexErrorBubbled or similar
    assert!(
        !result.errors.is_empty(),
        "lex-error-only source should surface at least one error"
    );
}

/// A chained comparison produces an error but parsing continues.
#[test]
fn chained_cmp_with_more_code() {
    let result = parse_str("let a = 1\nlet bad = a < b < c\nlet z = 99");
    assert!(
        result.errors.iter().any(|e| matches!(
            e,
            finlang_parser::ParseError::ChainedComparison { .. }
        )),
        "expected ChainedComparison error"
    );
    // `z` should still be parsed
    let has_z = result.items.iter().any(|item| {
        matches!(item, finlang_parser::ast::Item::LetDecl { name, .. } if name == "z")
    });
    assert!(has_z, "expected `let z = 99` to survive; items = {:?}", result.items);
}
