//! Operator precedence and AST shape tests.
//!
//! Each test parses a minimal expression and asserts the exact AST structure,
//! ensuring the precedence chain (or → and → cmp → add → mul → unary → cast
//! → postfix → primary) is wired correctly.

use finlang_parser::ast::{BinOpKind, Expr, Item, LiteralKind, TypeAnnotation, UnaryOpKind};
use finlang_parser::parse_str;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the single expression from an `ExprItem` file.
fn single_expr(src: &str) -> Expr {
    let result = parse_str(src);
    assert!(
        result.errors.is_empty(),
        "unexpected errors in '{src}': {:?}",
        result.errors
    );
    assert_eq!(result.items.len(), 1, "expected one item in '{src}'");
    match result.items.into_iter().next().unwrap() {
        Item::ExprItem(expr, _) => *expr,
        other => panic!("expected ExprItem, got {other:?}"),
    }
}

// ── Addition before multiplication ────────────────────────────────────────────

#[test]
fn add_mul_precedence() {
    // `1 + 2 * 3` must parse as `Add(1, Mul(2, 3))`
    let expr = single_expr("1 + 2 * 3");
    match expr {
        Expr::BinOp { op: BinOpKind::Add, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Literal(LiteralKind::Int(1), _)));
            match *rhs {
                Expr::BinOp { op: BinOpKind::Mul, lhs: l2, rhs: r2, .. } => {
                    assert!(matches!(*l2, Expr::Literal(LiteralKind::Int(2), _)));
                    assert!(matches!(*r2, Expr::Literal(LiteralKind::Int(3), _)));
                }
                other => panic!("rhs should be Mul, got {other:?}"),
            }
        }
        other => panic!("expected Add, got {other:?}"),
    }
}

#[test]
fn mul_add_precedence() {
    // `1 * 2 + 3` must parse as `Add(Mul(1, 2), 3)`
    let expr = single_expr("1 * 2 + 3");
    match expr {
        Expr::BinOp { op: BinOpKind::Add, lhs, rhs, .. } => {
            match *lhs {
                Expr::BinOp { op: BinOpKind::Mul, lhs: l2, rhs: r2, .. } => {
                    assert!(matches!(*l2, Expr::Literal(LiteralKind::Int(1), _)));
                    assert!(matches!(*r2, Expr::Literal(LiteralKind::Int(2), _)));
                }
                other => panic!("lhs should be Mul, got {other:?}"),
            }
            assert!(matches!(*rhs, Expr::Literal(LiteralKind::Int(3), _)));
        }
        other => panic!("expected Add, got {other:?}"),
    }
}

// ── Unary negation ────────────────────────────────────────────────────────────

#[test]
fn neg_add_precedence() {
    // `-1 + 2` must parse as `Add(Neg(1), 2)`
    let expr = single_expr("-1 + 2");
    match expr {
        Expr::BinOp { op: BinOpKind::Add, lhs, rhs, .. } => {
            match *lhs {
                Expr::UnaryOp { op: UnaryOpKind::Neg, expr, .. } => {
                    assert!(matches!(*expr, Expr::Literal(LiteralKind::Int(1), _)));
                }
                other => panic!("lhs should be Neg, got {other:?}"),
            }
            assert!(matches!(*rhs, Expr::Literal(LiteralKind::Int(2), _)));
        }
        other => panic!("expected Add, got {other:?}"),
    }
}

// ── Cast precedence ───────────────────────────────────────────────────────────

#[test]
fn cast_add_precedence() {
    // `a as price + b` must parse as `Add(Cast(a, Price), b)`
    let expr = single_expr("a as price + b");
    match expr {
        Expr::BinOp { op: BinOpKind::Add, lhs, rhs, .. } => {
            match *lhs {
                Expr::Cast { expr, ty: TypeAnnotation::Price, .. } => {
                    assert!(matches!(*expr, Expr::Ident(ref n, _) if n == "a"));
                }
                other => panic!("lhs should be Cast(a, Price), got {other:?}"),
            }
            assert!(matches!(*rhs, Expr::Ident(ref n, _) if n == "b"));
        }
        other => panic!("expected Add, got {other:?}"),
    }
}

#[test]
fn neg_cast_precedence() {
    // `-a as price` parses as `Neg(Cast(a, Price))` because unary wraps the
    // entire cast result (unary is level 6, cast is level 7).
    let expr = single_expr("-a as price");
    match expr {
        Expr::UnaryOp { op: UnaryOpKind::Neg, expr, .. } => {
            match *expr {
                Expr::Cast { expr: inner, ty: TypeAnnotation::Price, .. } => {
                    assert!(matches!(*inner, Expr::Ident(ref n, _) if n == "a"));
                }
                other => panic!("inner should be Cast(a, Price), got {other:?}"),
            }
        }
        other => panic!("expected Neg, got {other:?}"),
    }
}

// ── Callee must be a bare identifier ─────────────────────────────────────────
//
// `f(x)(y)` is not legal: only a bare identifier can be the callee.
// This is a documented grammar restriction — see `Expr::Call` doc comment.
// The second `(y)` is parsed as a separate expression item or yields an error.
// We test that `f(x)` alone does parse correctly.

#[test]
fn call_bare_ident() {
    let result = parse_str("f(x)");
    assert!(result.errors.is_empty());
    let expr = match result.items.into_iter().next().unwrap() {
        Item::ExprItem(e, _) => *e,
        other => panic!("expected ExprItem, got {other:?}"),
    };
    match expr {
        Expr::Call { name, args, .. } => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expr::Ident(ref n, _) if n == "x"));
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

// ── Index chaining ────────────────────────────────────────────────────────────

#[test]
fn nested_index() {
    // `arr[0][1]` → `Index(Index(arr, 0), 1)`
    let expr = single_expr("arr[0][1]");
    match expr {
        Expr::Index { expr: outer_expr, index: outer_idx, .. } => {
            assert!(matches!(*outer_idx, Expr::Literal(LiteralKind::Int(1), _)));
            match *outer_expr {
                Expr::Index { expr: inner_expr, index: inner_idx, .. } => {
                    assert!(matches!(*inner_expr, Expr::Ident(ref n, _) if n == "arr"));
                    assert!(matches!(*inner_idx, Expr::Literal(LiteralKind::Int(0), _)));
                }
                other => panic!("inner should be Index, got {other:?}"),
            }
        }
        other => panic!("expected Index, got {other:?}"),
    }
}

// ── Chained comparison → parse error ─────────────────────────────────────────

#[test]
fn chained_comparison_error() {
    // `a < b < c` must produce a ChainedComparison error but still return
    // *some* AST from error recovery.
    let result = parse_str("a < b < c");
    assert!(
        result.errors.iter().any(|e| matches!(
            e,
            finlang_parser::ParseError::ChainedComparison { .. }
        )),
        "expected ChainedComparison error, got: {:?}",
        result.errors
    );
    // recovery must produce at least one item
    assert!(!result.items.is_empty(), "expected recovered item after chained comparison error");
}

// ── Block with trailing expression ───────────────────────────────────────────

#[test]
fn block_with_tail() {
    // `{ let x = 1; x }` → Block([Let(x, 1)], Some(Ident("x")))
    let expr = single_expr("{ let x = 1; x }");
    match expr {
        Expr::Block(stmts, tail, _) => {
            assert_eq!(stmts.len(), 1, "expected one statement");
            assert!(
                matches!(stmts[0], finlang_parser::ast::Stmt::Let { ref name, .. } if name == "x"),
                "expected Let statement"
            );
            match tail {
                Some(t) => assert!(matches!(*t, Expr::Ident(ref n, _) if n == "x")),
                None => panic!("expected tail expression"),
            }
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn block_without_tail() {
    // `{ let x = 1; }` → Block([Let(x, 1)], None)
    let expr = single_expr("{ let x = 1; }");
    match expr {
        Expr::Block(stmts, tail, _) => {
            assert_eq!(stmts.len(), 1, "expected one statement");
            assert!(tail.is_none(), "expected no tail expression");
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

// ── Left-associative cast chain ───────────────────────────────────────────────

#[test]
fn cast_chain_left_assoc() {
    // `x as price as rate` → Cast(Cast(x, Price), Rate)
    let expr = single_expr("x as price as rate");
    match expr {
        Expr::Cast { expr: outer, ty: TypeAnnotation::Rate, .. } => {
            match *outer {
                Expr::Cast { expr: inner, ty: TypeAnnotation::Price, .. } => {
                    assert!(matches!(*inner, Expr::Ident(ref n, _) if n == "x"));
                }
                other => panic!("inner should be Cast(x, Price), got {other:?}"),
            }
        }
        other => panic!("expected outer Cast with Rate, got {other:?}"),
    }
}

// ── Boolean operators ─────────────────────────────────────────────────────────

#[test]
fn and_binds_tighter_than_or() {
    // `a || b && c` → Or(a, And(b, c))
    let expr = single_expr("a || b && c");
    match expr {
        Expr::BinOp { op: BinOpKind::Or, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Ident(ref n, _) if n == "a"));
            match *rhs {
                Expr::BinOp { op: BinOpKind::And, lhs: l2, rhs: r2, .. } => {
                    assert!(matches!(*l2, Expr::Ident(ref n, _) if n == "b"));
                    assert!(matches!(*r2, Expr::Ident(ref n, _) if n == "c"));
                }
                other => panic!("rhs should be And, got {other:?}"),
            }
        }
        other => panic!("expected Or, got {other:?}"),
    }
}
