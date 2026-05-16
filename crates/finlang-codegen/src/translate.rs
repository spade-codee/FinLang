//! IR → Cranelift IR translation.
//!
//! The public entry point is [`translate_function`], which lowers a single
//! [`IrFunction`] into the Cranelift [`cranelift_codegen::Context`] ready for
//! compilation by either the JIT or AOT engine.
//!
//! # Block-parameter / phi translation
//!
//! Cranelift uses *block parameters* instead of SSA φ-nodes.  The translator
//! handles this in two phases:
//!
//! 1. **Pre-scan** — before any instructions are emitted, every [`Inst::Phi`] at
//!    the top of each block is discovered and its destination value is registered
//!    as a block parameter via `FunctionBuilder::append_block_param`.
//!
//! 2. **Jump argument injection** — when emitting a `Jump` or the taken/not-taken
//!    arms of a `Branch`, the translator consults a pre-built map from
//!    `BlockId → Vec<ValueId>` (the block-parameter values expected by the target
//!    block) and passes them as jump arguments.
//!
//! If the program contains phi nodes that are *not* at the start of a block the
//! translator returns [`CodegenError::Unsupported`].
//!
//! The three example `.fin` files (`option_pricing`, `bond_portfolio`,
//! `var_calculation`) are all straight-line code: they emit only one block with
//! no branches.  Full phi support is present for completeness.

use std::collections::HashMap;

use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder,
};
use cranelift_codegen::ir::{FuncRef, Signature};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{FuncId, Linkage, Module};

use finlang_ir::{BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};
use finlang_parser::ast::{BinOpKind, UnaryOpKind};

use crate::common::{ir_type_to_clif, stdlib_signature, CodegenError};

// ── Signature helpers ─────────────────────────────────────────────────────────

/// Build a Cranelift [`Signature`] for an [`IrFunction`] using the module's
/// default calling convention.
pub fn ir_sig_to_clif<M: Module>(func: &IrFunction, module: &M) -> Signature {
    let mut sig = module.make_signature();
    for (_, _, ty) in &func.params {
        sig.params.push(AbiParam::new(ir_type_to_clif(*ty)));
    }
    sig.returns.push(AbiParam::new(ir_type_to_clif(func.return_ty)));
    sig
}

/// Build a Cranelift [`Signature`] from raw parameter/return [`IrType`] slices.
pub fn raw_sig_to_clif<M: Module>(
    params: &[IrType],
    ret: IrType,
    module: &M,
) -> Signature {
    let mut sig = module.make_signature();
    for &p in params {
        sig.params.push(AbiParam::new(ir_type_to_clif(p)));
    }
    sig.returns.push(AbiParam::new(ir_type_to_clif(ret)));
    sig
}

// ── Main translation entry point ──────────────────────────────────────────────

/// Translate a single [`IrFunction`] into Cranelift IR.
///
/// The function's Cranelift signature **must** already be set on `ctx.func`
/// before this is called (see [`ir_sig_to_clif`]).  After a successful return
/// the caller should call `module.define_function(func_id, ctx)` and then
/// `module.clear_context(ctx)`.
///
/// # Errors
///
/// Returns [`CodegenError::Unsupported`] if the function contains phi nodes
/// that are placed after non-phi instructions (a structural SSA invariant
/// violation), or any other unsupported IR construct.
///
/// Returns [`CodegenError::ModuleError`] on any Cranelift module operation
/// failure (e.g. duplicate function declaration).
///
/// Returns [`CodegenError::Internal`] on internal consistency failures that
/// indicate a bug in the upstream IR-lowering pass.
pub fn translate_function<M: Module>(
    func: &IrFunction,
    program: &IrProgram,
    module: &mut M,
    _func_id: FuncId,
    ctx: &mut cranelift_codegen::Context,
    fb_ctx: &mut FunctionBuilderContext,
) -> Result<(), CodegenError> {
    // ── 1. Build a FunctionBuilder over ctx.func ──────────────────────────────
    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);

    // ── 2. Pre-create all Cranelift blocks (one per IR BasicBlock) ────────────
    let mut block_map: HashMap<BlockId, cranelift_codegen::ir::Block> = HashMap::new();
    for bb in &func.blocks {
        let clif_block = builder.create_block();
        block_map.insert(bb.id, clif_block);
    }

    // ── 3. Pre-scan phis: collect block_id → Vec<(phi_dst_ValueId, clif_Value)>
    //    and register block parameters for each phi destination.
    //
    //    `phi_params[block_id]` is the ordered list of ValueIds that callers
    //    must pass as jump arguments when jumping to `block_id`.
    let mut phi_params: HashMap<BlockId, Vec<ValueId>> = HashMap::new();
    // value_map will grow as we translate each instruction.
    let mut value_map: HashMap<ValueId, cranelift_codegen::ir::Value> = HashMap::new();

    for bb in &func.blocks {
        let clif_block = block_map[&bb.id];
        let mut block_phi_dst: Vec<ValueId> = Vec::new();
        for inst in &bb.insts {
            match inst {
                Inst::Phi { dst, .. } => {
                    let ty = func
                        .value_types
                        .get(dst.0 as usize)
                        .copied()
                        .ok_or(CodegenError::Internal(format!(
                            "phi dst v{} has no type entry",
                            dst.0
                        )))?;
                    let clif_val =
                        builder.append_block_param(clif_block, ir_type_to_clif(ty));
                    value_map.insert(*dst, clif_val);
                    block_phi_dst.push(*dst);
                }
                // Phis must precede all non-phi instructions; stop at the first non-phi.
                _ => break,
            }
        }
        if !block_phi_dst.is_empty() {
            phi_params.insert(bb.id, block_phi_dst);
        }
    }

    // ── 4. Wire function parameters into the entry block ─────────────────────
    let entry_clif = block_map[&func.entry];
    builder.append_block_params_for_function_params(entry_clif);

    // Map each IR parameter ValueId → the corresponding entry-block parameter.
    let entry_params = builder.block_params(entry_clif).to_vec();
    for (idx, (_, vid, _)) in func.params.iter().enumerate() {
        let clif_val = entry_params.get(idx).copied().ok_or_else(|| {
            CodegenError::Internal(format!("param v{} has no block param at index {idx}", vid.0))
        })?;
        value_map.insert(*vid, clif_val);
    }

    // ── 5. Per-block: build "phi sources" map for jump-argument injection ─────
    //
    // For each block J that has phi nodes, every predecessor block P must pass
    // the correct value when it jumps to J.  We build:
    //
    //   phi_args_for_jump: (src_block_id, target_block_id) → Vec<ValueId>
    //
    // populated from each Phi's `incoming` list.
    let mut phi_args_for_jump: HashMap<(BlockId, BlockId), Vec<ValueId>> = HashMap::new();

    for bb in &func.blocks {
        for inst in &bb.insts {
            if let Inst::Phi { dst: _, incoming } = inst {
                for (val_from_pred, pred_block_id) in incoming {
                    phi_args_for_jump
                        .entry((*pred_block_id, bb.id))
                        .or_default()
                        .push(*val_from_pred);
                }
            }
        }
    }

    // ── 6. Cache for callee FuncIds (avoid re-declaring the same function) ────
    let mut callee_cache: HashMap<String, FuncId> = HashMap::new();

    // ── 7. Translate each block ───────────────────────────────────────────────
    for bb in &func.blocks {
        let clif_block = block_map[&bb.id];
        builder.switch_to_block(clif_block);

        // Seal the entry block immediately (no back-edges lead to it).
        // Other blocks are sealed lazily after all predecessors have been filled.
        // We use seal_all_blocks() at the end; block sealing happens there.

        let mut saw_non_phi = false;

        for inst in &bb.insts {
            match inst {
                // ── Constants ─────────────────────────────────────────────────
                Inst::ConstF64 { dst, value } => {
                    saw_non_phi = true;
                    let v = builder.ins().f64const(*value);
                    value_map.insert(*dst, v);
                }
                Inst::ConstI64 { dst, value } => {
                    saw_non_phi = true;
                    let v = builder.ins().iconst(types::I64, *value);
                    value_map.insert(*dst, v);
                }
                Inst::ConstBool { dst, value } => {
                    saw_non_phi = true;
                    let v = builder.ins().iconst(types::I8, i64::from(*value));
                    value_map.insert(*dst, v);
                }

                // ── Phi (block parameter — already wired in pre-scan) ─────────
                Inst::Phi { .. } => {
                    if saw_non_phi {
                        return Err(CodegenError::Unsupported(
                            "phi node after non-phi instruction",
                        ));
                    }
                    // Already handled in the pre-scan; nothing more to do here.
                }

                // ── Binary operations ─────────────────────────────────────────
                Inst::BinOp { dst, op, lhs, rhs } => {
                    saw_non_phi = true;
                    let lv = lookup_value(&value_map, *lhs)?;
                    let rv = lookup_value(&value_map, *rhs)?;
                    let lhs_ty = func
                        .value_types
                        .get(lhs.0 as usize)
                        .copied()
                        .ok_or_else(|| {
                            CodegenError::Internal(format!("v{} has no type", lhs.0))
                        })?;

                    let result = emit_binop(&mut builder, *op, lv, rv, lhs_ty)?;
                    value_map.insert(*dst, result);
                }

                // ── Unary operations ──────────────────────────────────────────
                Inst::UnaryOp { dst, op, operand } => {
                    saw_non_phi = true;
                    let ov = lookup_value(&value_map, *operand)?;
                    let operand_ty = func
                        .value_types
                        .get(operand.0 as usize)
                        .copied()
                        .ok_or_else(|| {
                            CodegenError::Internal(format!("v{} has no type", operand.0))
                        })?;
                    let result = emit_unary(&mut builder, *op, ov, operand_ty)?;
                    value_map.insert(*dst, result);
                }

                // ── Casts ─────────────────────────────────────────────────────
                Inst::CastIntToFloat { dst, src } => {
                    saw_non_phi = true;
                    let sv = lookup_value(&value_map, *src)?;
                    let v = builder.ins().fcvt_from_sint(types::F64, sv);
                    value_map.insert(*dst, v);
                }
                Inst::CastFloatToInt { dst, src } => {
                    saw_non_phi = true;
                    let sv = lookup_value(&value_map, *src)?;
                    let v = builder.ins().fcvt_to_sint(types::I64, sv);
                    value_map.insert(*dst, v);
                }

                // ── Calls ─────────────────────────────────────────────────────
                Inst::Call { dst, callee, args } => {
                    saw_non_phi = true;
                    let func_ref = resolve_callee(
                        callee,
                        func,
                        program,
                        module,
                        &mut builder,
                        &mut callee_cache,
                    )?;

                    let arg_vals: Result<Vec<_>, _> = args
                        .iter()
                        .map(|&vid| lookup_value(&value_map, vid))
                        .collect();
                    let arg_vals = arg_vals?;

                    let call_inst = builder.ins().call(func_ref, &arg_vals);
                    if let Some(dst_vid) = dst {
                        let results = builder.inst_results(call_inst);
                        let result_val = results.first().copied().ok_or_else(|| {
                            CodegenError::Internal(format!(
                                "call to {callee} returned no values but dst v{} expected",
                                dst_vid.0
                            ))
                        })?;
                        value_map.insert(*dst_vid, result_val);
                    }
                }

                // ── Terminators ───────────────────────────────────────────────
                Inst::Return { value } => {
                    saw_non_phi = true;
                    match value {
                        Some(vid) => {
                            let v = lookup_value(&value_map, *vid)?;
                            builder.ins().return_(&[v]);
                        }
                        None => {
                            builder.ins().return_(&[]);
                        }
                    }
                }

                Inst::Jump { target } => {
                    saw_non_phi = true;
                    let target_clif = block_map[target];
                    let jump_args = build_jump_args(
                        bb.id,
                        *target,
                        &phi_args_for_jump,
                        &value_map,
                    )?;
                    builder.ins().jump(target_clif, &jump_args);
                }

                Inst::Branch { cond, then_block, else_block } => {
                    saw_non_phi = true;
                    let cond_val = lookup_value(&value_map, *cond)?;
                    let then_clif = block_map[then_block];
                    let else_clif = block_map[else_block];

                    let then_args = build_jump_args(
                        bb.id,
                        *then_block,
                        &phi_args_for_jump,
                        &value_map,
                    )?;
                    let else_args = build_jump_args(
                        bb.id,
                        *else_block,
                        &phi_args_for_jump,
                        &value_map,
                    )?;

                    builder.ins().brif(
                        cond_val,
                        then_clif,
                        &then_args,
                        else_clif,
                        &else_args,
                    );
                }
            }
        }
    }

    // ── 8. Seal all blocks and finalise ──────────────────────────────────────
    builder.seal_all_blocks();
    builder.finalize();

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Look up a Cranelift value in `value_map`, returning an error if not found.
fn lookup_value(
    value_map: &HashMap<ValueId, cranelift_codegen::ir::Value>,
    vid: ValueId,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    value_map.get(&vid).copied().ok_or_else(|| {
        CodegenError::Internal(format!("use of undefined SSA value v{}", vid.0))
    })
}

/// Build the ordered list of Cranelift jump arguments when jumping from
/// `src_block` to `target_block`.
fn build_jump_args(
    src_block: BlockId,
    target_block: BlockId,
    phi_args_for_jump: &HashMap<(BlockId, BlockId), Vec<ValueId>>,
    value_map: &HashMap<ValueId, cranelift_codegen::ir::Value>,
) -> Result<Vec<cranelift_codegen::ir::Value>, CodegenError> {
    match phi_args_for_jump.get(&(src_block, target_block)) {
        None => Ok(Vec::new()),
        Some(vids) => vids
            .iter()
            .map(|&vid| lookup_value(value_map, vid))
            .collect(),
    }
}

/// Emit a binary operation instruction and return the result value.
fn emit_binop(
    builder: &mut FunctionBuilder<'_>,
    op: BinOpKind,
    lv: cranelift_codegen::ir::Value,
    rv: cranelift_codegen::ir::Value,
    lhs_ty: IrType,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    match lhs_ty {
        IrType::F64 => match op {
            BinOpKind::Add  => Ok(builder.ins().fadd(lv, rv)),
            BinOpKind::Sub  => Ok(builder.ins().fsub(lv, rv)),
            BinOpKind::Mul  => Ok(builder.ins().fmul(lv, rv)),
            BinOpKind::Div  => Ok(builder.ins().fdiv(lv, rv)),
            BinOpKind::Mod  => Err(CodegenError::Unsupported("% on f64 is not defined")),
            BinOpKind::Eq   => Ok(builder.ins().fcmp(FloatCC::Equal,            lv, rv)),
            BinOpKind::NotEq => Ok(builder.ins().fcmp(FloatCC::NotEqual,        lv, rv)),
            BinOpKind::Lt   => Ok(builder.ins().fcmp(FloatCC::LessThan,         lv, rv)),
            BinOpKind::Gt   => Ok(builder.ins().fcmp(FloatCC::GreaterThan,      lv, rv)),
            BinOpKind::LtEq => Ok(builder.ins().fcmp(FloatCC::LessThanOrEqual,  lv, rv)),
            BinOpKind::GtEq => Ok(builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lv, rv)),
            BinOpKind::And | BinOpKind::Or => {
                Err(CodegenError::Unsupported("logical and/or on f64"))
            }
        },
        IrType::I64 => match op {
            BinOpKind::Add  => Ok(builder.ins().iadd(lv, rv)),
            BinOpKind::Sub  => Ok(builder.ins().isub(lv, rv)),
            BinOpKind::Mul  => Ok(builder.ins().imul(lv, rv)),
            BinOpKind::Div  => Ok(builder.ins().sdiv(lv, rv)),
            BinOpKind::Mod  => Ok(builder.ins().srem(lv, rv)),
            BinOpKind::Eq   => Ok(builder.ins().icmp(IntCC::Equal,          lv, rv)),
            BinOpKind::NotEq => Ok(builder.ins().icmp(IntCC::NotEqual,      lv, rv)),
            BinOpKind::Lt   => Ok(builder.ins().icmp(IntCC::SignedLessThan, lv, rv)),
            BinOpKind::Gt   => Ok(builder.ins().icmp(IntCC::SignedGreaterThan, lv, rv)),
            BinOpKind::LtEq => Ok(builder.ins().icmp(IntCC::SignedLessThanOrEqual, lv, rv)),
            BinOpKind::GtEq => Ok(builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, lv, rv)),
            BinOpKind::And | BinOpKind::Or => {
                Err(CodegenError::Unsupported("logical and/or on i64"))
            }
        },
        IrType::Bool => match op {
            BinOpKind::And => Ok(builder.ins().band(lv, rv)),
            BinOpKind::Or  => Ok(builder.ins().bor(lv, rv)),
            BinOpKind::Eq  => Ok(builder.ins().icmp(IntCC::Equal,    lv, rv)),
            BinOpKind::NotEq => Ok(builder.ins().icmp(IntCC::NotEqual, lv, rv)),
            other => Err(CodegenError::Unsupported(binop_name_for_bool(other))),
        },
    }
}

/// Return a static string naming a `BinOpKind` for error messages.
fn binop_name_for_bool(op: BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add  => "add on bool",
        BinOpKind::Sub  => "sub on bool",
        BinOpKind::Mul  => "mul on bool",
        BinOpKind::Div  => "div on bool",
        BinOpKind::Mod  => "mod on bool",
        BinOpKind::Lt   => "lt on bool",
        BinOpKind::Gt   => "gt on bool",
        BinOpKind::LtEq => "lteq on bool",
        BinOpKind::GtEq => "gteq on bool",
        _ => "unsupported binop on bool",
    }
}

/// Emit a unary operation instruction and return the result value.
fn emit_unary(
    builder: &mut FunctionBuilder<'_>,
    op: UnaryOpKind,
    ov: cranelift_codegen::ir::Value,
    operand_ty: IrType,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    match (op, operand_ty) {
        (UnaryOpKind::Neg, IrType::F64)  => Ok(builder.ins().fneg(ov)),
        (UnaryOpKind::Neg, IrType::I64)  => Ok(builder.ins().ineg(ov)),
        (UnaryOpKind::Not, IrType::Bool) => {
            // Boolean NOT: XOR with 1 (flips the least-significant bit of the i8).
            Ok(builder.ins().bxor_imm(ov, 1))
        }
        (UnaryOpKind::Neg, IrType::Bool) => {
            Err(CodegenError::Unsupported("neg on bool"))
        }
        (UnaryOpKind::Not, _) => {
            Err(CodegenError::Unsupported("logical not on non-bool"))
        }
    }
}

/// Declare (or retrieve from cache) a callee function reference inside the
/// currently-being-built function.
///
/// The lookup order is:
/// 1. `stdlib_signature` — known `finlang_*` extern symbols.
/// 2. The program's own function list — user-defined functions.
fn resolve_callee<M: Module>(
    callee: &str,
    _current_func: &IrFunction,
    program: &IrProgram,
    module: &mut M,
    builder: &mut FunctionBuilder<'_>,
    callee_cache: &mut HashMap<String, FuncId>,
) -> Result<FuncRef, CodegenError> {
    // Look up (or declare) the callee's FuncId.
    let func_id = if let Some(cached_id) = callee_cache.get(callee) {
        *cached_id
    } else {
        // Build the Cranelift signature.
        let clif_sig = if let Some((params, ret)) = stdlib_signature(callee) {
            raw_sig_to_clif(params, ret, module)
        } else {
            // User-defined function — find it in the program.
            let callee_ir = program
                .functions
                .iter()
                .find(|f| f.name == callee)
                .ok_or_else(|| {
                    CodegenError::Internal(format!("unknown callee '{callee}'"))
                })?;
            ir_sig_to_clif(callee_ir, module)
        };

        // Determine linkage: stdlib symbols are imported; user functions are exported.
        let linkage = if stdlib_signature(callee).is_some() {
            Linkage::Import
        } else {
            Linkage::Export
        };

        let id = module
            .declare_function(callee, linkage, &clif_sig)
            .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

        callee_cache.insert(callee.to_owned(), id);
        id
    };

    // Obtain a FuncRef valid inside the current function being built.
    let func_ref = module.declare_func_in_func(func_id, builder.func);

    Ok(func_ref)
}
