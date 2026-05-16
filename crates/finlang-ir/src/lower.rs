//! AST → SSA IR lowering pass.
//!
//! The entry point is [`lower`], which accepts a slice of type-checked
//! top-level [`Item`]s and the [`TypeCheckResult`] carrying the per-expression
//! type map, and returns an [`IrProgram`].
//!
//! # Architecture
//!
//! Lowering is driven by a [`FunctionBuilder`] that accumulates blocks and
//! instructions for one function at a time.  The builder owns:
//!
//! * A monotonically increasing `ValueId` counter.
//! * A `BlockId` counter.
//! * A stack of `HashMap<String, ValueId>` environment frames (pushed/popped
//!   around blocks and function bodies).
//! * The partially-built [`IrFunction`].
//!
//! Top-level `let` declarations and the trailing expression of a source file
//! are lowered into a synthetic `__main__` function.  User-defined `fn` items
//! and `portfolio` blocks become their own [`IrFunction`]s.

use std::collections::HashMap;

use finlang_parser::ast::{
    BinOpKind, Expr, Item, LiteralKind, Stmt, TypeAnnotation, UnaryOpKind,
};
use finlang_types::ty::{annotation_to_fin_type, FinType};
use finlang_types::TypeCheckResult;

use crate::error::LowerError;
use crate::ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};

// ── Public entry point ────────────────────────────────────────────────────────

/// Lower a fully type-checked FinLang program to SSA IR.
///
/// The `items` slice must have been produced by `finlang_parser::parse_str` and
/// subsequently verified by `finlang_types::check` with zero errors.  Passing
/// items with type errors may cause `LowerError::MissingTypeInfo` or
/// `LowerError::UnsupportedType` returns, but will never panic.
///
/// # Errors
///
/// Returns `Err` for language constructs that the IR does not support
/// (e.g. `for` loops, list literals) or for internal consistency failures
/// that indicate compiler bugs.
///
/// # Examples
///
/// ```no_run
/// use finlang_ir::lower;
/// use finlang_types::check;
/// use finlang_parser::parse_str;
///
/// let parsed = parse_str("let x: price = 5.0 as price\nx");
/// let types  = check(&parsed.items);
/// let prog   = lower(&parsed.items, &types).unwrap();
/// assert_eq!(prog.functions[0].name, "__main__");
/// ```
pub fn lower(items: &[Item], types: &TypeCheckResult) -> Result<IrProgram, LowerError> {
    let mut prog = IrProgram { functions: Vec::new() };
    let mut lowerer = Lowerer { types };

    // Separate top-level lets / expr-items (→ __main__) from fn / portfolio defs.
    let mut main_items: Vec<&Item> = Vec::new();
    let mut other_items: Vec<&Item> = Vec::new();

    for item in items {
        match item {
            Item::FnDef { .. } | Item::PortfolioDef { .. } => other_items.push(item),
            Item::LetDecl { .. } | Item::ExprItem(_, _) => main_items.push(item),
        }
    }

    // Lower user-defined functions and portfolio blocks first so that
    // __main__ can (in principle) call them.
    for item in &other_items {
        match item {
            Item::FnDef { name, params, return_ty, body, .. } => {
                let fn_ir = lowerer.lower_fn_def(name, params, return_ty, body)?;
                prog.functions.push(fn_ir);
            }
            Item::PortfolioDef { name, legs, .. } => {
                let fn_ir = lowerer.lower_portfolio(name, legs)?;
                prog.functions.push(fn_ir);
            }
            _ => unreachable!(),
        }
    }

    // Lower __main__ last so it can reference globals.
    if !main_items.is_empty() {
        let main_fn = lowerer.lower_main(&main_items)?;
        prog.functions.insert(0, main_fn);
    }

    Ok(prog)
}

// ── Type mapping ──────────────────────────────────────────────────────────────

/// Map a [`FinType`] to the IR's flat [`IrType`].
///
/// All financial numeric dimensions collapse to `F64`.  `Int`, `Date`, and
/// `OptionType` become `I64`.  Compound types and error-recovery sentinels
/// are rejected.
pub(crate) fn fintype_to_ir(t: &FinType) -> Result<IrType, LowerError> {
    match t {
        FinType::Price
        | FinType::Rate
        | FinType::Notional
        | FinType::Years
        | FinType::BasisPoints => Ok(IrType::F64),
        // Numeric is the unresolved-literal sentinel; treat defensively as F64.
        FinType::Numeric => Ok(IrType::F64),
        FinType::Int | FinType::Date | FinType::OptionType => Ok(IrType::I64),
        FinType::Bool => Ok(IrType::Bool),
        FinType::List(_) | FinType::Fn(_, _) | FinType::Unknown => {
            Err(LowerError::UnsupportedType(t.clone()))
        }
    }
}

/// Map a parsed [`TypeAnnotation`] to an [`IrType`].
fn annotation_to_ir(ann: &TypeAnnotation) -> Result<IrType, LowerError> {
    fintype_to_ir(&annotation_to_fin_type(ann))
}

// ── Lowerer ───────────────────────────────────────────────────────────────────

struct Lowerer<'tcx> {
    types: &'tcx TypeCheckResult,
}

impl<'tcx> Lowerer<'tcx> {
    // ── __main__ ──────────────────────────────────────────────────────────────

    fn lower_main(&mut self, items: &[&Item]) -> Result<IrFunction, LowerError> {
        // Determine the return type from the last ExprItem.
        let ret_fin = items
            .iter()
            .rev()
            .find_map(|it| {
                if let Item::ExprItem(e, sp) = it {
                    self.types.expr_types.get(sp).or_else(|| {
                        // Fall back to expression span itself.
                        self.types.expr_types.get(&e.span())
                    })
                } else {
                    None
                }
            })
            .cloned()
            .unwrap_or(FinType::Numeric);

        let ret_ir = fintype_to_ir(&ret_fin).unwrap_or(IrType::F64);

        let mut builder = FunctionBuilder::new("__main__".to_owned(), vec![], ret_ir);

        let mut last_value: Option<ValueId> = None;

        for item in items {
            match item {
                Item::LetDecl { name, value, .. } => {
                    let vid = self.lower_expr(value, &mut builder)?;
                    builder.bind(name.clone(), vid);
                    last_value = Some(vid);
                }
                Item::ExprItem(expr, _) => {
                    let vid = self.lower_expr(expr, &mut builder)?;
                    last_value = Some(vid);
                }
                _ => unreachable!(),
            }
        }

        builder.emit(Inst::Return { value: last_value });
        Ok(builder.finish())
    }

    // ── fn def ────────────────────────────────────────────────────────────────

    fn lower_fn_def(
        &mut self,
        name: &str,
        params: &[finlang_parser::ast::Param],
        return_ty: &TypeAnnotation,
        body: &Expr,
    ) -> Result<IrFunction, LowerError> {
        let ret_ir = annotation_to_ir(return_ty)?;

        let mut ir_params: Vec<(String, IrType)> = Vec::new();
        for p in params {
            let pty = annotation_to_ir(&p.ty)?;
            ir_params.push((p.name.clone(), pty));
        }

        let mut builder = FunctionBuilder::new(name.to_owned(), ir_params, ret_ir);

        let val = self.lower_expr(body, &mut builder)?;
        builder.emit(Inst::Return { value: Some(val) });

        Ok(builder.finish())
    }

    // ── portfolio ─────────────────────────────────────────────────────────────

    fn lower_portfolio(
        &mut self,
        name: &str,
        _legs: &[finlang_parser::ast::PortfolioLeg],
    ) -> Result<IrFunction, LowerError> {
        // TODO: resolve instrument identifiers for real leg pricing.
        // For now emit a stub that returns 0.0 (price).
        let fn_name = format!("__portfolio_{name}__");
        let mut builder = FunctionBuilder::new(fn_name, vec![], IrType::F64);
        let zero = builder.alloc(IrType::F64);
        builder.emit(Inst::ConstF64 { dst: zero, value: 0.0 });
        builder.emit(Inst::Return { value: Some(zero) });
        Ok(builder.finish())
    }

    // ── Expression lowering ───────────────────────────────────────────────────

    fn lower_expr(
        &mut self,
        expr: &Expr,
        b: &mut FunctionBuilder,
    ) -> Result<ValueId, LowerError> {
        match expr {
            // ── Literals ──────────────────────────────────────────────────────
            Expr::Literal(kind, _span) => {
                match kind {
                    LiteralKind::Float(f) => {
                        let dst = b.alloc(IrType::F64);
                        b.emit(Inst::ConstF64 { dst, value: *f });
                        Ok(dst)
                    }
                    LiteralKind::Int(i) => {
                        let dst = b.alloc(IrType::I64);
                        b.emit(Inst::ConstI64 { dst, value: *i });
                        Ok(dst)
                    }
                    LiteralKind::Bool(bv) => {
                        let dst = b.alloc(IrType::Bool);
                        b.emit(Inst::ConstBool { dst, value: *bv });
                        Ok(dst)
                    }
                    LiteralKind::Call => {
                        // Discriminant 0 per the stdlib ABI.
                        let dst = b.alloc(IrType::I64);
                        b.emit(Inst::ConstI64 { dst, value: 0 }); // Call discriminant
                        Ok(dst)
                    }
                    LiteralKind::Put => {
                        // Discriminant 1 per the stdlib ABI.
                        let dst = b.alloc(IrType::I64);
                        b.emit(Inst::ConstI64 { dst, value: 1 }); // Put discriminant
                        Ok(dst)
                    }
                    LiteralKind::String(_) => {
                        Err(LowerError::UnsupportedConstruct("string literal"))
                    }
                }
            }

            // ── Identifier ────────────────────────────────────────────────────
            Expr::Ident(name, _span) => {
                b.lookup(name)
                    .ok_or_else(|| LowerError::UndefinedVariable(name.clone()))
            }

            // ── Binary op ─────────────────────────────────────────────────────
            Expr::BinOp { op, lhs, rhs, span } => {
                let lhs_v = self.lower_expr(lhs, b)?;
                let rhs_v = self.lower_expr(rhs, b)?;

                // Use the type checker's result type for the dst.
                let result_fin = self
                    .types
                    .expr_types
                    .get(span)
                    .cloned()
                    .unwrap_or(FinType::Numeric);
                let result_ty = fintype_to_ir(&result_fin).unwrap_or(IrType::F64);

                // Comparison / logical operators produce bool regardless.
                let dst_ty = match op {
                    BinOpKind::Eq
                    | BinOpKind::NotEq
                    | BinOpKind::Lt
                    | BinOpKind::Gt
                    | BinOpKind::LtEq
                    | BinOpKind::GtEq
                    | BinOpKind::And
                    | BinOpKind::Or => IrType::Bool,
                    _ => result_ty,
                };

                let dst = b.alloc(dst_ty);
                b.emit(Inst::BinOp { dst, op: *op, lhs: lhs_v, rhs: rhs_v });
                Ok(dst)
            }

            // ── Unary op ──────────────────────────────────────────────────────
            Expr::UnaryOp { op, expr: inner, span } => {
                let operand = self.lower_expr(inner, b)?;

                let result_fin = self
                    .types
                    .expr_types
                    .get(span)
                    .cloned()
                    .unwrap_or(FinType::Numeric);
                let dst_ty = match op {
                    UnaryOpKind::Not => IrType::Bool,
                    UnaryOpKind::Neg => fintype_to_ir(&result_fin).unwrap_or(IrType::F64),
                };

                let dst = b.alloc(dst_ty);
                b.emit(Inst::UnaryOp { dst, op: *op, operand });
                Ok(dst)
            }

            // ── Function call ─────────────────────────────────────────────────
            Expr::Call { name, args, span } => {
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    let v = self.lower_expr(arg, b)?;
                    arg_vals.push(v);
                }

                // Determine return type from type checker.
                let ret_fin = self
                    .types
                    .expr_types
                    .get(span)
                    .cloned()
                    .unwrap_or(FinType::Numeric);
                let ret_ty = fintype_to_ir(&ret_fin).unwrap_or(IrType::F64);

                let dst = b.alloc(ret_ty);
                // Mangle: stdlib functions are prefixed with `finlang_`.
                // User-defined functions keep their name as-is.
                let callee = mangle_callee(name);
                b.emit(Inst::Call { dst: Some(dst), callee, args: arg_vals });
                Ok(dst)
            }

            // ── Cast ──────────────────────────────────────────────────────────
            Expr::Cast { expr: inner, ty, span } => {
                let src_v = self.lower_expr(inner, b)?;
                let src_ty = b.value_type(src_v);

                let target_fin = annotation_to_fin_type(ty);
                let target_ty = fintype_to_ir(&target_fin)?;

                // Use span type if available (handles Numeric resolution).
                let effective_target = self
                    .types
                    .expr_types
                    .get(span)
                    .and_then(|ft| fintype_to_ir(ft).ok())
                    .unwrap_or(target_ty);

                match (src_ty, effective_target) {
                    // Same primitive type: no-op cast (financial dimension change only).
                    (a, b) if a == b => Ok(src_v),
                    (IrType::I64, IrType::F64) => {
                        let dst = b.alloc(IrType::F64);
                        b.emit(Inst::CastIntToFloat { dst, src: src_v });
                        Ok(dst)
                    }
                    (IrType::F64, IrType::I64) => {
                        let dst = b.alloc(IrType::I64);
                        b.emit(Inst::CastFloatToInt { dst, src: src_v });
                        Ok(dst)
                    }
                    // Bool ↔ anything: unsupported (type checker rejects these).
                    _ => Ok(src_v),
                }
            }

            // ── If expression ─────────────────────────────────────────────────
            Expr::If { cond, then_branch, else_branch, span } => {
                let else_branch = else_branch.as_ref().ok_or(
                    LowerError::UnsupportedConstruct("if without else as expression"),
                )?;

                // Determine result type.
                let result_fin = self
                    .types
                    .expr_types
                    .get(span)
                    .cloned()
                    .unwrap_or(FinType::Numeric);
                let result_ty = fintype_to_ir(&result_fin).unwrap_or(IrType::F64);

                let cond_v = self.lower_expr(cond, b)?;

                // Allocate three new blocks.
                let then_bb = b.new_block();
                let else_bb = b.new_block();
                let join_bb = b.new_block();

                // Terminate current block with conditional branch.
                b.emit(Inst::Branch {
                    cond: cond_v,
                    then_block: then_bb,
                    else_block: else_bb,
                });

                // --- then arm ---
                b.switch_to(then_bb);
                let then_val = self.lower_expr(then_branch, b)?;
                let then_pred = b.current_block();
                b.emit(Inst::Jump { target: join_bb });

                // --- else arm ---
                b.switch_to(else_bb);
                let else_val = self.lower_expr(else_branch, b)?;
                let else_pred = b.current_block();
                b.emit(Inst::Jump { target: join_bb });

                // --- join block with phi ---
                b.switch_to(join_bb);
                let phi_dst = b.alloc(result_ty);
                b.emit(Inst::Phi {
                    dst: phi_dst,
                    incoming: vec![(then_val, then_pred), (else_val, else_pred)],
                });
                Ok(phi_dst)
            }

            // ── Block ─────────────────────────────────────────────────────────
            Expr::Block(stmts, tail, _span) => {
                b.push_scope();
                for stmt in stmts {
                    self.lower_stmt(stmt, b)?;
                }
                let result = if let Some(tail_expr) = tail {
                    self.lower_expr(tail_expr, b)?
                } else {
                    // Empty block: emit 0.0.
                    let dst = b.alloc(IrType::F64);
                    b.emit(Inst::ConstF64 { dst, value: 0.0 });
                    dst
                };
                b.pop_scope();
                Ok(result)
            }

            // ── Unsupported ───────────────────────────────────────────────────
            Expr::List(_, _) => Err(LowerError::UnsupportedConstruct("list literal")),
            Expr::Index { .. } => Err(LowerError::UnsupportedConstruct("list indexing")),
        }
    }

    // ── Statement lowering ────────────────────────────────────────────────────

    fn lower_stmt(
        &mut self,
        stmt: &Stmt,
        b: &mut FunctionBuilder,
    ) -> Result<(), LowerError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let vid = self.lower_expr(value, b)?;
                b.bind(name.clone(), vid);
            }
            Stmt::Expr(expr, _) => {
                self.lower_expr(expr, b)?;
            }
            Stmt::Return(maybe_expr, _) => {
                let val = maybe_expr
                    .as_ref()
                    .map(|e| self.lower_expr(e, b))
                    .transpose()?;
                b.emit(Inst::Return { value: val });
            }
            Stmt::For { .. } => {
                return Err(LowerError::UnsupportedConstruct("for loop"));
            }
        }
        Ok(())
    }
}

// ── Callee mangling ───────────────────────────────────────────────────────────

/// Mangle a FinLang callee name to its ABI symbol.
///
/// Stdlib functions (`black_scholes`, `bond_price`, etc.) get the
/// `finlang_` prefix.  User-defined functions keep their original name.
fn mangle_callee(name: &str) -> String {
    use finlang_types::stdlib_sigs::lookup_stdlib;
    if lookup_stdlib(name).is_some() {
        format!("finlang_{name}")
    } else {
        name.to_owned()
    }
}

// ── FunctionBuilder ───────────────────────────────────────────────────────────

/// Incremental builder for a single [`IrFunction`].
///
/// The builder tracks the current basic block, allocates `ValueId`s and
/// `BlockId`s, maintains a lexical environment stack, and finally assembles
/// the finished [`IrFunction`] via [`FunctionBuilder::finish`].
struct FunctionBuilder {
    /// The function being built.
    func: IrFunction,
    /// Counter for `ValueId` allocation.
    next_value: u32,
    /// Counter for `BlockId` allocation.
    next_block: u32,
    /// Index into `func.blocks` for the currently active block.
    current_block_idx: usize,
    /// Lexical environment: stack of `name → ValueId` scopes.
    env: Vec<HashMap<String, ValueId>>,
}

impl FunctionBuilder {
    /// Create a new builder for a function.
    ///
    /// `params` is a list of `(name, ir_type)` pairs.  The builder pre-allocates
    /// a `ValueId` for each parameter and stores them in `IrFunction::params`.
    fn new(
        name: String,
        params: Vec<(String, IrType)>,
        return_ty: IrType,
    ) -> Self {
        let entry = BlockId(0);
        let mut func = IrFunction {
            name,
            params: Vec::new(),
            return_ty,
            blocks: vec![BasicBlock { id: entry, insts: Vec::new() }],
            entry,
            value_types: Vec::new(),
        };

        let mut next_value: u32 = 0;
        let mut env_frame: HashMap<String, ValueId> = HashMap::new();

        for (param_name, param_ty) in params {
            let vid = ValueId(next_value);
            next_value += 1;
            func.value_types.push(param_ty);
            func.params.push((param_name.clone(), vid, param_ty));
            env_frame.insert(param_name, vid);
        }

        Self {
            func,
            next_value,
            next_block: 1,
            current_block_idx: 0,
            env: vec![env_frame],
        }
    }

    /// Allocate a fresh `ValueId` with the given type.
    fn alloc(&mut self, ty: IrType) -> ValueId {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.func.value_types.push(ty);
        vid
    }

    /// Create a new (empty) basic block and return its `BlockId`.
    fn new_block(&mut self) -> BlockId {
        let bid = BlockId(self.next_block);
        self.next_block += 1;
        self.func.blocks.push(BasicBlock { id: bid, insts: Vec::new() });
        bid
    }

    /// Switch the active block to `bid`.
    fn switch_to(&mut self, bid: BlockId) {
        self.current_block_idx = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == bid)
            .expect("block id must exist");
    }

    /// Return the id of the currently active block.
    fn current_block(&self) -> BlockId {
        self.func.blocks[self.current_block_idx].id
    }

    /// Append an instruction to the current block.
    fn emit(&mut self, inst: Inst) {
        self.func.blocks[self.current_block_idx].insts.push(inst);
    }

    /// Bind a name to a value in the innermost scope.
    fn bind(&mut self, name: String, vid: ValueId) {
        if let Some(frame) = self.env.last_mut() {
            frame.insert(name, vid);
        }
    }

    /// Look up a name by walking from the innermost to outermost scope.
    fn lookup(&self, name: &str) -> Option<ValueId> {
        for frame in self.env.iter().rev() {
            if let Some(&vid) = frame.get(name) {
                return Some(vid);
            }
        }
        None
    }

    /// Push a new lexical scope.
    fn push_scope(&mut self) {
        self.env.push(HashMap::new());
    }

    /// Pop the innermost lexical scope.
    fn pop_scope(&mut self) {
        if self.env.len() > 1 {
            self.env.pop();
        }
    }

    /// Return the recorded type of a value.
    ///
    /// # Panics
    ///
    /// Panics if `vid` was not allocated by this builder — this is an
    /// internal compiler invariant.
    fn value_type(&self, vid: ValueId) -> IrType {
        self.func.value_types[vid.0 as usize]
    }

    /// Consume the builder and return the finished [`IrFunction`].
    fn finish(self) -> IrFunction {
        self.func
    }
}
