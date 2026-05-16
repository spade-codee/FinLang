//! Constant-folding pass.
//!
//! [`const_fold`] runs to a fixed point over every function: as long as one
//! iteration folds at least one instruction, it runs another pass.  This
//! handles chains like `(1.0 + 2.0) * 3.0` where the inner addition must fold
//! before the multiplication can.
//!
//! # Scope
//!
//! The pass is **intra-block, straight-line only**.  It does not propagate
//! constants across `Phi` nodes, `Call` instructions, or block boundaries.
//! Full sparse conditional constant propagation (SCCP) would be a natural
//! next step, but is not implemented here.

use std::collections::HashMap;

use finlang_parser::ast::{BinOpKind, UnaryOpKind};

use crate::ir::{Inst, IrFunction, IrProgram, ValueId};

// ── Constant lattice ──────────────────────────────────────────────────────────

/// A known-constant value carried by a `ValueId`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ConstLit {
    /// A 64-bit float constant.
    F64(f64),
    /// A 64-bit integer constant.
    I64(i64),
    /// A boolean constant.
    Bool(bool),
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Constant-fold all functions in `program` to a fixed point.
///
/// Each `BinOp` or `UnaryOp` whose operands are all known constants is
/// replaced by the corresponding `Const*` instruction.  The pass loops until
/// no further folding is possible.
///
/// IEEE-754 edge cases (NaN, infinity) are handled correctly — `0.0 / 0.0`
/// produces a `ConstF64(NaN)` rather than panicking.
pub fn const_fold(program: &mut IrProgram) {
    for func in &mut program.functions {
        fold_function(func);
    }
}

fn fold_function(func: &mut IrFunction) {
    loop {
        let changed = fold_pass(func);
        if !changed {
            break;
        }
    }
}

/// Run one folding pass over all blocks.  Returns `true` if anything changed.
fn fold_pass(func: &mut IrFunction) -> bool {
    // Build an initial constant map from all existing Const* instructions.
    let mut consts: HashMap<ValueId, ConstLit> = HashMap::new();
    for block in func.blocks.iter() {
        for inst in &block.insts {
            seed_const(&mut consts, inst);
        }
    }

    let mut changed = false;
    for block in &mut func.blocks {
        for inst in &mut block.insts {
            if let Some(replacement) = try_fold(inst, &consts) {
                // Record the new constant before replacing.
                seed_const(&mut consts, &replacement);
                *inst = replacement;
                changed = true;
            }
        }
    }
    changed
}

/// Populate the constant map from a single instruction.
fn seed_const(consts: &mut HashMap<ValueId, ConstLit>, inst: &Inst) {
    match inst {
        Inst::ConstF64 { dst, value } => {
            consts.insert(*dst, ConstLit::F64(*value));
        }
        Inst::ConstI64 { dst, value } => {
            consts.insert(*dst, ConstLit::I64(*value));
        }
        Inst::ConstBool { dst, value } => {
            consts.insert(*dst, ConstLit::Bool(*value));
        }
        _ => {}
    }
}

/// Attempt to fold one instruction.
///
/// Returns `Some(replacement)` if the instruction can be replaced by a
/// constant, or `None` if it cannot be folded.
fn try_fold(inst: &Inst, consts: &HashMap<ValueId, ConstLit>) -> Option<Inst> {
    match inst {
        Inst::BinOp { dst, op, lhs, rhs } => {
            let lv = consts.get(lhs)?;
            let rv = consts.get(rhs)?;
            fold_binop(*dst, *op, *lv, *rv)
        }
        Inst::UnaryOp { dst, op, operand } => {
            let v = consts.get(operand)?;
            fold_unary(*dst, *op, *v)
        }
        _ => None,
    }
}

// ── Arithmetic evaluation ─────────────────────────────────────────────────────

fn fold_binop(dst: ValueId, op: BinOpKind, lv: ConstLit, rv: ConstLit) -> Option<Inst> {
    match (lv, rv) {
        (ConstLit::F64(l), ConstLit::F64(r)) => Some(fold_f64_binop(dst, op, l, r)),
        (ConstLit::I64(l), ConstLit::I64(r)) => fold_i64_binop(dst, op, l, r),
        (ConstLit::Bool(l), ConstLit::Bool(r)) => fold_bool_binop(dst, op, l, r),
        // Mixed types should not arise in well-typed IR; skip folding.
        _ => None,
    }
}

fn fold_f64_binop(dst: ValueId, op: BinOpKind, l: f64, r: f64) -> Inst {
    match op {
        BinOpKind::Add => Inst::ConstF64 { dst, value: l + r },
        BinOpKind::Sub => Inst::ConstF64 { dst, value: l - r },
        BinOpKind::Mul => Inst::ConstF64 { dst, value: l * r },
        // IEEE-754: 0.0/0.0 = NaN, x/0.0 = ±inf — all fine as constants.
        BinOpKind::Div => Inst::ConstF64 { dst, value: l / r },
        BinOpKind::Mod => Inst::ConstF64 { dst, value: l % r },
        BinOpKind::Eq => Inst::ConstBool { dst, value: l == r },
        BinOpKind::NotEq => Inst::ConstBool { dst, value: l != r },
        BinOpKind::Lt => Inst::ConstBool { dst, value: l < r },
        BinOpKind::Gt => Inst::ConstBool { dst, value: l > r },
        BinOpKind::LtEq => Inst::ConstBool { dst, value: l <= r },
        BinOpKind::GtEq => Inst::ConstBool { dst, value: l >= r },
        // Boolean ops on floats don't arise in well-typed IR; produce false defensively.
        BinOpKind::And | BinOpKind::Or => Inst::ConstBool { dst, value: false },
    }
}

fn fold_i64_binop(dst: ValueId, op: BinOpKind, l: i64, r: i64) -> Option<Inst> {
    Some(match op {
        BinOpKind::Add => Inst::ConstI64 { dst, value: l.wrapping_add(r) },
        BinOpKind::Sub => Inst::ConstI64 { dst, value: l.wrapping_sub(r) },
        BinOpKind::Mul => Inst::ConstI64 { dst, value: l.wrapping_mul(r) },
        BinOpKind::Div => {
            if r == 0 {
                return None; // Division by zero: skip folding.
            }
            Inst::ConstI64 { dst, value: l.wrapping_div(r) }
        }
        BinOpKind::Mod => {
            if r == 0 {
                return None;
            }
            Inst::ConstI64 { dst, value: l.wrapping_rem(r) }
        }
        BinOpKind::Eq => Inst::ConstBool { dst, value: l == r },
        BinOpKind::NotEq => Inst::ConstBool { dst, value: l != r },
        BinOpKind::Lt => Inst::ConstBool { dst, value: l < r },
        BinOpKind::Gt => Inst::ConstBool { dst, value: l > r },
        BinOpKind::LtEq => Inst::ConstBool { dst, value: l <= r },
        BinOpKind::GtEq => Inst::ConstBool { dst, value: l >= r },
        BinOpKind::And | BinOpKind::Or => Inst::ConstBool { dst, value: false },
    })
}

fn fold_bool_binop(dst: ValueId, op: BinOpKind, l: bool, r: bool) -> Option<Inst> {
    Some(match op {
        BinOpKind::And => Inst::ConstBool { dst, value: l && r },
        BinOpKind::Or => Inst::ConstBool { dst, value: l || r },
        BinOpKind::Eq => Inst::ConstBool { dst, value: l == r },
        BinOpKind::NotEq => Inst::ConstBool { dst, value: l != r },
        _ => return None,
    })
}

fn fold_unary(dst: ValueId, op: UnaryOpKind, v: ConstLit) -> Option<Inst> {
    match (op, v) {
        (UnaryOpKind::Neg, ConstLit::F64(f)) => Some(Inst::ConstF64 { dst, value: -f }),
        (UnaryOpKind::Neg, ConstLit::I64(i)) => {
            Some(Inst::ConstI64 { dst, value: i.wrapping_neg() })
        }
        (UnaryOpKind::Not, ConstLit::Bool(b)) => Some(Inst::ConstBool { dst, value: !b }),
        _ => None,
    }
}
