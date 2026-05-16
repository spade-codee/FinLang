//! The single-pass AST type-checking walk.
//!
//! [`TypeChecker`] walks every [`Item`] in the AST exactly once (top to
//! bottom, left to right within expressions) and produces a
//! [`TypeCheckResult`].
//!
//! # Scope model
//!
//! Variables live in a [`SymbolTable`] — a stack of `HashMap<String, FinType>`
//! scopes.  The stack is pushed on entering a block, function body, for-loop
//! body, or the synthetic scope around each top-level `let`; it is popped on
//! exit.  Globals (top-level `let`s) stay in the bottom scope so they remain
//! accessible throughout the file.
//!
//! # Literal inference
//!
//! Bare numeric literals get type [`FinType::Numeric`].  The checker resolves
//! this in three situations:
//!
//! 1. **Expected-type context** — `let x: price = 5.0` immediately coerces
//!    `5.0` to `Price`.
//! 2. **Cast** — `5.0 as price` → `Price`.
//! 3. **Binary-op rules** — `Numeric * rate` → `Rate` via the static table.
//!
//! If `Numeric` survives to a top-level binding or expression statement it
//! triggers [`TypeError::UnresolvedLiteralType`].

use std::collections::HashMap;

use finlang_lexer::Span;
use finlang_parser::ast::{Expr, Item, LiteralKind, PortfolioLeg, Stmt, UnaryOpKind};

use crate::error::TypeError;
use crate::rules::lookup_rule;
use crate::stdlib_sigs::lookup_stdlib;
use crate::ty::{annotation_to_fin_type, FinType};

// ── Symbol table ──────────────────────────────────────────────────────────────

/// A lexically-scoped symbol table.
///
/// The underlying structure is a `Vec<HashMap<String, FinType>>`.  The last
/// entry is the innermost (current) scope; index `0` is the module-level
/// (global) scope.
#[derive(Debug)]
pub struct SymbolTable {
    scopes: Vec<HashMap<String, FinType>>,
}

impl SymbolTable {
    /// Create a new symbol table with one (global) scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Push a new scope (on entering a block, function, or loop).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope (on exiting a block, function, or loop).
    ///
    /// The global scope is never popped; this is a no-op when only one scope
    /// remains.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Insert `name → ty` in the **innermost** scope.
    pub fn insert(&mut self, name: String, ty: FinType) {
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name, ty);
        }
    }

    /// Look up `name` walking from the innermost scope to the outermost.
    ///
    /// Returns `None` if the name is not declared in any enclosing scope.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&FinType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

// ── TypeChecker ───────────────────────────────────────────────────────────────

/// Internal type-checker state.
struct TypeChecker {
    errors: Vec<TypeError>,
    expr_types: HashMap<Span, FinType>,
    symbols: SymbolTable,
}

impl TypeChecker {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            expr_types: HashMap::new(),
            symbols: SymbolTable::new(),
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Record the inferred type of an expression node.
    fn record(&mut self, span: Span, ty: FinType) {
        self.expr_types.insert(span, ty);
    }

    /// Record an error and return `FinType::Unknown` for use in recovery.
    fn error(&mut self, e: TypeError) -> FinType {
        self.errors.push(e);
        FinType::Unknown
    }

    /// Resolve `Numeric` to `expected` when the context demands a specific type.
    ///
    /// If the actual type is already `expected` (or `Unknown` for recovery),
    /// return it unchanged.  If it's `Numeric` and `expected` is a numeric
    /// dimension, coerce.  Otherwise emit a type mismatch (used at call sites).
    fn coerce_numeric(&self, actual: FinType, expected: &FinType) -> FinType {
        if actual == FinType::Unknown || *expected == FinType::Unknown {
            return FinType::Unknown;
        }
        if actual == FinType::Numeric && is_numeric_dim(expected) {
            expected.clone()
        } else {
            actual
        }
    }

    // ── Item checking ─────────────────────────────────────────────────────────

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::LetDecl { name, ty, value, span } => {
                let expected = ty.as_ref().map(annotation_to_fin_type);
                let val_ty = self.check_expr(value, expected.as_ref());
                let bound_ty = expected.unwrap_or_else(|| val_ty.clone());
                // Warn on unresolved Numeric at the top level.
                if bound_ty == FinType::Numeric {
                    self.errors
                        .push(TypeError::UnresolvedLiteralType { span: value.span() });
                }
                self.record(*span, bound_ty.clone());
                self.symbols.insert(name.clone(), bound_ty);
            }

            Item::FnDef { name, params, return_ty, body, span } => {
                let ret_ty = annotation_to_fin_type(return_ty);
                // Build function type and register it so the function is
                // callable recursively / from later declarations.
                let param_types: Vec<FinType> = params
                    .iter()
                    .map(|p| annotation_to_fin_type(&p.ty))
                    .collect();
                let fn_ty = FinType::Fn(param_types.clone(), Box::new(ret_ty.clone()));
                self.symbols.insert(name.clone(), fn_ty);

                // Enter function scope.
                self.symbols.push_scope();
                for (p, pty) in params.iter().zip(param_types.iter()) {
                    self.symbols.insert(p.name.clone(), pty.clone());
                }
                let body_ty = self.check_expr(body, Some(&ret_ty));
                self.symbols.pop_scope();

                self.record(*span, ret_ty.clone());
                // Check body returns the declared type (ignoring Unknown which
                // means there was already an error inside).
                if body_ty != ret_ty
                    && body_ty != FinType::Unknown
                    && !(body_ty == FinType::Numeric && is_numeric_dim(&ret_ty))
                {
                    // We don't have a dedicated "return type mismatch" error in
                    // the spec, so reuse IfBranchMismatch shape — or just emit
                    // a Dimensional error on the body span.
                    self.errors.push(TypeError::IfBranchMismatch {
                        then_ty: body_ty,
                        else_ty: ret_ty,
                        span: body.span(),
                    });
                }
            }

            Item::PortfolioDef { name: _, legs, span } => {
                self.symbols.push_scope();
                for leg in legs {
                    self.check_portfolio_leg(leg);
                }
                self.symbols.pop_scope();
                self.record(*span, FinType::Unknown);
            }

            Item::ExprItem(expr, span) => {
                let ty = self.check_expr(expr, None);
                if ty == FinType::Numeric {
                    self.errors
                        .push(TypeError::UnresolvedLiteralType { span: *span });
                }
                self.record(*span, ty);
            }
        }
    }

    fn check_portfolio_leg(&mut self, leg: &PortfolioLeg) {
        // The size expression is most commonly `N as notional` or similar.
        self.check_expr(&leg.size, None);
        for (_name, val) in &leg.at_clauses {
            self.check_expr(val, None);
        }
    }

    // ── Statement checking ────────────────────────────────────────────────────

    /// Type-check a statement, returning the type it contributes (or Unknown).
    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, value, span } => {
                let expected = ty.as_ref().map(annotation_to_fin_type);
                let val_ty = self.check_expr(value, expected.as_ref());
                let bound_ty = if let Some(ann) = expected {
                    // Annotation overrides inference (Numeric coerces to the declared type).
                    ann
                } else {
                    val_ty.clone()
                };
                if bound_ty == FinType::Numeric {
                    self.errors
                        .push(TypeError::UnresolvedLiteralType { span: value.span() });
                }
                self.record(*span, bound_ty.clone());
                self.symbols.insert(name.clone(), bound_ty);
            }

            Stmt::Expr(expr, span) => {
                let ty = self.check_expr(expr, None);
                self.record(*span, ty);
            }

            Stmt::Return(maybe_expr, span) => {
                if let Some(expr) = maybe_expr {
                    let ty = self.check_expr(expr, None);
                    self.record(*span, ty);
                } else {
                    self.record(*span, FinType::Unknown);
                }
            }

            Stmt::For { var, iter, body, span } => {
                let iter_ty = self.check_expr(iter, None);
                // Determine the element type.
                let elem_ty = match &iter_ty {
                    FinType::List(inner) => *inner.clone(),
                    FinType::Unknown => FinType::Unknown,
                    other => {
                        self.errors.push(TypeError::IndexedNonList {
                            found: other.clone(),
                            span: iter.span(),
                        });
                        FinType::Unknown
                    }
                };
                self.symbols.push_scope();
                self.symbols.insert(var.clone(), elem_ty);
                self.check_expr(body, None);
                self.symbols.pop_scope();
                self.record(*span, FinType::Unknown);
            }
        }
    }

    // ── Expression checking ───────────────────────────────────────────────────

    /// Type-check an expression.
    ///
    /// `expected` is the type demanded by the surrounding context.  It is used
    /// only to coerce `Numeric` literals; it does **not** suppress genuine
    /// type errors.
    fn check_expr(&mut self, expr: &Expr, expected: Option<&FinType>) -> FinType {
        let ty = self.check_expr_inner(expr, expected);
        self.record(expr.span(), ty.clone());
        ty
    }

    fn check_expr_inner(&mut self, expr: &Expr, expected: Option<&FinType>) -> FinType {
        match expr {
            // ── Literals ──────────────────────────────────────────────────────
            Expr::Literal(kind, _span) => match kind {
                LiteralKind::Bool(_) => FinType::Bool,
                LiteralKind::Call | LiteralKind::Put => FinType::OptionType,
                LiteralKind::String(_) => FinType::Unknown, // strings not in the type system
                LiteralKind::Int(_) | LiteralKind::Float(_) => {
                    // If context demands a numeric dimension, use it directly.
                    if let Some(exp) = expected {
                        if is_numeric_dim(exp) {
                            return exp.clone();
                        }
                    }
                    FinType::Numeric
                }
            },

            // ── Identifiers ───────────────────────────────────────────────────
            Expr::Ident(name, span) => {
                if let Some(ty) = self.symbols.lookup(name) {
                    let ty = ty.clone();
                    // Coerce Numeric identifier when context demands.
                    if ty == FinType::Numeric {
                        if let Some(exp) = expected {
                            if is_numeric_dim(exp) {
                                return exp.clone();
                            }
                        }
                    }
                    ty
                } else {
                    self.error(TypeError::UnknownIdentifier {
                        name: name.clone(),
                        span: *span,
                    })
                }
            }

            // ── Binary operations ─────────────────────────────────────────────
            Expr::BinOp { op, lhs, rhs, span } => {
                // No expected-type propagation into operands of a BinOp —
                // each operand stands on its own.  The result is determined
                // by the rules table.
                let lhs_ty = self.check_expr(lhs, None);
                let rhs_ty = self.check_expr(rhs, None);

                match lookup_rule(&lhs_ty, *op, &rhs_ty) {
                    Some(rule) => match rule.result {
                        Ok(scalar) => scalar.to_fin_type(),
                        Err(msg) => self.error(TypeError::Dimensional {
                            lhs: lhs_ty,
                            op: *op,
                            rhs: rhs_ty,
                            lhs_span: lhs.span(),
                            rhs_span: rhs.span(),
                            span: *span,
                            custom_msg: Some(msg),
                        }),
                    },
                    None => self.error(TypeError::Dimensional {
                        lhs: lhs_ty,
                        op: *op,
                        rhs: rhs_ty,
                        lhs_span: lhs.span(),
                        rhs_span: rhs.span(),
                        span: *span,
                        custom_msg: None,
                    }),
                }
            }

            // ── Unary operations ──────────────────────────────────────────────
            Expr::UnaryOp { op, expr: inner, span: _ } => {
                let inner_ty = self.check_expr(inner, expected);
                match op {
                    UnaryOpKind::Neg => {
                        // Negation is valid on any numeric dimension.
                        if is_numeric_dim(&inner_ty)
                            || inner_ty == FinType::Numeric
                            || inner_ty == FinType::Unknown
                        {
                            inner_ty
                        } else {
                            // Produce Unknown — we could emit an error but the
                            // spec doesn't define a specific unary error variant.
                            FinType::Unknown
                        }
                    }
                    UnaryOpKind::Not => {
                        if inner_ty == FinType::Bool || inner_ty == FinType::Unknown {
                            FinType::Bool
                        } else {
                            FinType::Unknown
                        }
                    }
                }
            }

            // ── Function calls ────────────────────────────────────────────────
            Expr::Call { name, args, span } => {
                self.check_call(name, args, *span)
            }

            // ── Casts ─────────────────────────────────────────────────────────
            Expr::Cast { expr: inner, ty, span } => {
                let target = annotation_to_fin_type(ty);
                let inner_ty = self.check_expr(inner, Some(&target));
                self.check_cast(inner_ty, target, *span)
            }

            // ── If expressions ────────────────────────────────────────────────
            Expr::If { cond, then_branch, else_branch, span } => {
                let cond_ty = self.check_expr(cond, Some(&FinType::Bool));
                if cond_ty != FinType::Bool
                    && cond_ty != FinType::Unknown
                    && cond_ty != FinType::Numeric
                {
                    self.errors.push(TypeError::IfConditionNotBool {
                        found: cond_ty,
                        span: cond.span(),
                    });
                }

                let then_ty = self.check_expr(then_branch, expected);
                let else_ty = else_branch
                    .as_ref()
                    .map(|b| self.check_expr(b, expected))
                    .unwrap_or(FinType::Unknown);

                if let Some(else_expr) = else_branch {
                    let _ = else_expr; // already checked above
                    // If both branches are typed and disagree, emit an error.
                    if then_ty != FinType::Unknown
                        && else_ty != FinType::Unknown
                        && !types_compatible(&then_ty, &else_ty)
                    {
                        self.errors.push(TypeError::IfBranchMismatch {
                            then_ty: then_ty.clone(),
                            else_ty: else_ty.clone(),
                            span: *span,
                        });
                        FinType::Unknown
                    } else {
                        // Prefer the more specific type (resolve Numeric).
                        merge_types(then_ty, else_ty)
                    }
                } else {
                    then_ty
                }
            }

            // ── Blocks ────────────────────────────────────────────────────────
            Expr::Block(stmts, tail, _span) => {
                self.symbols.push_scope();
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                let ty = if let Some(tail_expr) = tail {
                    self.check_expr(tail_expr, expected)
                } else {
                    FinType::Unknown
                };
                self.symbols.pop_scope();
                ty
            }

            // ── Index expressions ─────────────────────────────────────────────
            Expr::Index { expr: coll, index, span: _ } => {
                let coll_ty = self.check_expr(coll, None);
                let idx_ty = self.check_expr(index, Some(&FinType::Int));

                if idx_ty != FinType::Int
                    && idx_ty != FinType::Numeric
                    && idx_ty != FinType::Unknown
                {
                    self.errors.push(TypeError::IndexNotInt {
                        found: idx_ty,
                        span: index.span(),
                    });
                }

                match &coll_ty {
                    FinType::List(inner) => *inner.clone(),
                    FinType::Unknown => FinType::Unknown,
                    other => {
                        self.errors.push(TypeError::IndexedNonList {
                            found: other.clone(),
                            span: coll.span(),
                        });
                        FinType::Unknown
                    }
                }
            }

            // ── List literals ─────────────────────────────────────────────────
            Expr::List(elems, _span) => {
                if elems.is_empty() {
                    // Empty list — type is inferred from context or Unknown.
                    if let Some(FinType::List(inner)) = expected {
                        return FinType::List(inner.clone());
                    }
                    return FinType::List(Box::new(FinType::Unknown));
                }

                // The element type context comes from the outer expected type.
                let elem_expected = if let Some(FinType::List(inner)) = expected {
                    Some(inner.as_ref())
                } else {
                    None
                };

                let first_ty = self.check_expr(&elems[0], elem_expected);
                for (i, elem) in elems.iter().enumerate().skip(1) {
                    let elem_ty = self.check_expr(elem, Some(&first_ty));
                    let resolved = self.coerce_numeric(elem_ty.clone(), &first_ty);
                    if resolved != first_ty
                        && resolved != FinType::Unknown
                        && first_ty != FinType::Unknown
                    {
                        self.errors.push(TypeError::ListElementMismatch {
                            expected: first_ty.clone(),
                            found: elem_ty,
                            index: i,
                            span: elem.span(),
                        });
                    }
                }

                FinType::List(Box::new(first_ty))
            }
        }
    }

    // ── Call checking ─────────────────────────────────────────────────────────

    fn check_call(&mut self, name: &str, args: &[Expr], span: Span) -> FinType {
        // Check local scope first (user-defined functions).
        if let Some(FinType::Fn(param_types, ret)) = self.symbols.lookup(name).cloned() {
            if args.len() != param_types.len() {
                return self.error(TypeError::WrongArity {
                    fn_name: name.to_owned(),
                    expected: param_types.len(),
                    found: args.len(),
                    span,
                });
            }
            for (i, (arg, expected)) in args.iter().zip(param_types.iter()).enumerate() {
                let arg_ty = self.check_expr(arg, Some(expected));
                let resolved = self.coerce_numeric(arg_ty.clone(), expected);
                if resolved != *expected && resolved != FinType::Unknown {
                    self.errors.push(TypeError::MismatchedArgument {
                        fn_name: name.to_owned(),
                        arg_index: i,
                        expected: expected.clone(),
                        found: arg_ty,
                        span: arg.span(),
                    });
                }
            }
            return *ret;
        }

        // Then check stdlib.
        if let Some(sig) = lookup_stdlib(name) {
            if args.len() != sig.params.len() {
                return self.error(TypeError::WrongArity {
                    fn_name: name.to_owned(),
                    expected: sig.params.len(),
                    found: args.len(),
                    span,
                });
            }
            for (i, (arg, expected)) in args.iter().zip(sig.params.iter()).enumerate() {
                let arg_ty = self.check_expr(arg, Some(expected));
                let resolved = self.coerce_numeric(arg_ty.clone(), expected);
                if resolved != *expected && resolved != FinType::Unknown {
                    self.errors.push(TypeError::MismatchedArgument {
                        fn_name: name.to_owned(),
                        arg_index: i,
                        expected: expected.clone(),
                        found: arg_ty,
                        span: arg.span(),
                    });
                }
            }
            return sig.ret.clone();
        }

        self.error(TypeError::UnknownFunction {
            name: name.to_owned(),
            span,
        })
    }

    // ── Cast checking ─────────────────────────────────────────────────────────

    fn check_cast(&mut self, from: FinType, to: FinType, span: Span) -> FinType {
        // Test whether the cast is forbidden before checking the general
        // escape-hatch arms, so error arms always fire correctly.
        let forbidden = self.cast_is_forbidden(&from, &to);
        if let Some((bad_from, bad_to)) = forbidden {
            return self.error(TypeError::InvalidCast {
                from: bad_from,
                to: bad_to,
                span,
            });
        }

        match (&from, &to) {
            // Unknown propagates.
            (FinType::Unknown, _) | (_, FinType::Unknown) => to,

            // Same type.
            (a, b) if a == b => to,

            // List(T) → List(U): recurse.
            (FinType::List(inner_from), FinType::List(inner_to)) => {
                let inner_from = *inner_from.clone();
                let inner_to = *inner_to.clone();
                let result = self.check_cast(inner_from, inner_to.clone(), span);
                if result == FinType::Unknown {
                    FinType::Unknown
                } else {
                    FinType::List(Box::new(inner_to))
                }
            }

            // All other casts are allowed (`as` is the escape hatch).
            _ => to,
        }
    }

    /// Returns `Some((from, to))` if this cast is explicitly forbidden,
    /// or `None` if it should be allowed.
    fn cast_is_forbidden(&self, from: &FinType, to: &FinType) -> Option<(FinType, FinType)> {
        // Numeric or Unknown: never forbidden (Numeric is the widening literal type).
        if *from == FinType::Unknown || *to == FinType::Unknown || *from == FinType::Numeric {
            return None;
        }

        // Bool → numeric: forbidden.
        if *from == FinType::Bool && is_numeric_dim(to) {
            return Some((from.clone(), to.clone()));
        }

        // numeric → Bool: forbidden.
        if *to == FinType::Bool && is_numeric_dim(from) {
            return Some((from.clone(), to.clone()));
        }

        // numeric / Int → OptionType: forbidden.
        if *to == FinType::OptionType && (is_numeric_dim(from) || *from == FinType::Int) {
            return Some((from.clone(), to.clone()));
        }

        None
    }
}

// ── Helper predicates ─────────────────────────────────────────────────────────

/// Return `true` if `ty` is a numeric financial dimension (or `Int`).
///
/// `Bool`, `OptionType`, `List`, `Fn`, and `Unknown` are not numeric.
fn is_numeric_dim(ty: &FinType) -> bool {
    matches!(
        ty,
        FinType::Price
            | FinType::Rate
            | FinType::Notional
            | FinType::Years
            | FinType::BasisPoints
            | FinType::Int
            | FinType::Numeric
    )
}

/// Return `true` if two types are considered compatible (one may be `Numeric`).
fn types_compatible(a: &FinType, b: &FinType) -> bool {
    if a == b {
        return true;
    }
    if *a == FinType::Numeric && is_numeric_dim(b) {
        return true;
    }
    if *b == FinType::Numeric && is_numeric_dim(a) {
        return true;
    }
    false
}

/// Pick the more specific type: prefer a concrete type over `Numeric`.
fn merge_types(a: FinType, b: FinType) -> FinType {
    match (&a, &b) {
        (FinType::Numeric, _) => b,
        (_, FinType::Numeric) => a,
        _ => a,
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// The result of type-checking a FinLang program.
#[derive(Debug)]
pub struct TypeCheckResult {
    /// Type errors found.  Empty iff the program is well-typed.
    pub errors: Vec<TypeError>,
    /// Inferred type of each AST expression node, keyed by its source span.
    ///
    /// Used by the LSP for hover-type display.
    pub expr_types: HashMap<Span, FinType>,
}

/// Type-check a pre-parsed list of top-level items.
///
/// # Examples
///
/// ```rust
/// use finlang_types::check_str;
///
/// let result = check_str("let x: price = 5.0 as price");
/// assert!(result.errors.is_empty());
/// ```
#[must_use]
pub fn check(program: &[Item]) -> TypeCheckResult {
    let mut tc = TypeChecker::new();
    for item in program {
        tc.check_item(item);
    }
    TypeCheckResult {
        errors: tc.errors,
        expr_types: tc.expr_types,
    }
}

/// Parse `source` and type-check it in one step.
///
/// # Examples
///
/// ```rust
/// use finlang_types::check_str;
///
/// let r = check_str("let x: rate = 0.05");
/// assert!(r.errors.is_empty());
/// ```
#[must_use]
pub fn check_str(source: &str) -> TypeCheckResult {
    let parsed = finlang_parser::parse_str(source);
    check(&parsed.items)
}

/// Check the `BasisPoints / 10000.0` pattern used in `bond_portfolio.fin`.
///
/// The literal `10000.0` gets type `Numeric`, and the rule
/// `BasisPoints / Numeric = Rate` is present in the table.  This function
/// validates that the pattern is handled correctly.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_points_div_numeric_is_rate() {
        let r = check_str("let bp: basis_points = 1.0 as basis_points\nlet r: rate = bp / 10000.0");
        assert!(r.errors.is_empty(), "{:#?}", r.errors);
    }
}
