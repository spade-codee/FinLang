//! Unit tests for the constant-folding pass.
//!
//! Programs are hand-constructed directly in IR to avoid dependence on the
//! lowering pass.

use finlang_ir::ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};
use finlang_ir::opt::const_fold;
use finlang_parser::ast::{BinOpKind, UnaryOpKind};

fn make_program(insts: Vec<Inst>, return_val: ValueId) -> IrProgram {
    let mut all_insts = insts;
    all_insts.push(Inst::Return { value: Some(return_val) });

    // Build value_types: scan all instructions for dsts.
    let max_vid = all_insts
        .iter()
        .filter_map(|i| i.dst())
        .map(|v| v.0)
        .max()
        .unwrap_or(0);

    let mut value_types = vec![IrType::F64; (max_vid + 1) as usize];
    for inst in &all_insts {
        match inst {
            Inst::ConstI64 { dst, .. } => value_types[dst.0 as usize] = IrType::I64,
            Inst::ConstBool { dst, .. } => value_types[dst.0 as usize] = IrType::Bool,
            Inst::BinOp {
                dst,
                op:
                    BinOpKind::Eq
                    | BinOpKind::NotEq
                    | BinOpKind::Lt
                    | BinOpKind::Gt
                    | BinOpKind::LtEq
                    | BinOpKind::GtEq
                    | BinOpKind::And
                    | BinOpKind::Or,
                ..
            } => {
                value_types[dst.0 as usize] = IrType::Bool;
            }
            _ => {}
        }
    }

    IrProgram {
        functions: vec![IrFunction {
            name: "test".to_owned(),
            params: vec![],
            return_ty: IrType::F64,
            blocks: vec![BasicBlock { id: BlockId(0), insts: all_insts }],
            entry: BlockId(0),
            value_types,
        }],
    }
}

// ── f64 arithmetic ────────────────────────────────────────────────────────────

#[test]
fn fold_add_f64() {
    // v0 = 1.0, v1 = 2.0, v2 = v0 + v1  →  v2 = 3.0
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let mut prog = make_program(
        vec![
            Inst::ConstF64 { dst: v0, value: 1.0 },
            Inst::ConstF64 { dst: v1, value: 2.0 },
            Inst::BinOp { dst: v2, op: BinOpKind::Add, lhs: v0, rhs: v1 },
        ],
        v2,
    );
    const_fold(&mut prog);

    let folded = &prog.functions[0].blocks[0].insts[2];
    assert_eq!(*folded, Inst::ConstF64 { dst: v2, value: 3.0 });
}

#[test]
fn fold_chain_add_then_mul() {
    // v0=1.0, v1=2.0, v2=v0+v1, v3=3.0, v4=v2*v3  →  v4=9.0
    let (v0, v1, v2, v3, v4) =
        (ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4));
    let mut prog = make_program(
        vec![
            Inst::ConstF64 { dst: v0, value: 1.0 },
            Inst::ConstF64 { dst: v1, value: 2.0 },
            Inst::BinOp { dst: v2, op: BinOpKind::Add, lhs: v0, rhs: v1 },
            Inst::ConstF64 { dst: v3, value: 3.0 },
            Inst::BinOp { dst: v4, op: BinOpKind::Mul, lhs: v2, rhs: v3 },
        ],
        v4,
    );
    const_fold(&mut prog);

    let folded = &prog.functions[0].blocks[0].insts[4];
    assert_eq!(*folded, Inst::ConstF64 { dst: v4, value: 9.0 });
}

#[test]
fn fold_div_zero_is_nan_not_panic() {
    // v0=0.0, v1=0.0, v2=v0/v1  →  v2=NaN (must not panic)
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let mut prog = make_program(
        vec![
            Inst::ConstF64 { dst: v0, value: 0.0 },
            Inst::ConstF64 { dst: v1, value: 0.0 },
            Inst::BinOp { dst: v2, op: BinOpKind::Div, lhs: v0, rhs: v1 },
        ],
        v2,
    );
    const_fold(&mut prog);

    let folded = &prog.functions[0].blocks[0].insts[2];
    if let Inst::ConstF64 { value, .. } = folded {
        assert!(value.is_nan(), "expected NaN, got {value}");
    } else {
        panic!("expected ConstF64, got {folded:?}");
    }
}

// ── Unary negation ────────────────────────────────────────────────────────────

#[test]
fn fold_neg_f64() {
    // v0=5.0, v1=-v0  →  v1=-5.0
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let mut prog = make_program(
        vec![
            Inst::ConstF64 { dst: v0, value: 5.0 },
            Inst::UnaryOp { dst: v1, op: UnaryOpKind::Neg, operand: v0 },
        ],
        v1,
    );
    const_fold(&mut prog);

    let folded = &prog.functions[0].blocks[0].insts[1];
    assert_eq!(*folded, Inst::ConstF64 { dst: v1, value: -5.0 });
}

// ── Call boundary — folding must not cross it ─────────────────────────────────

#[test]
fn fold_does_not_cross_call() {
    // v0=1.0, call returns v1, v2 = v0 + v1.
    // v1 is not a constant so v2 must NOT be folded.
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let mut prog = make_program(
        vec![
            Inst::ConstF64 { dst: v0, value: 1.0 },
            Inst::Call {
                dst: Some(v1),
                callee: "finlang_discount_factor".to_owned(),
                args: vec![v0, v0],
            },
            Inst::BinOp { dst: v2, op: BinOpKind::Add, lhs: v0, rhs: v1 },
        ],
        v2,
    );
    const_fold(&mut prog);

    // v2 must still be a BinOp (not a constant).
    let maybe_folded = &prog.functions[0].blocks[0].insts[2];
    assert!(
        matches!(maybe_folded, Inst::BinOp { .. }),
        "folded across call boundary: {maybe_folded:?}"
    );
}

// ── Boolean folding ───────────────────────────────────────────────────────────

#[test]
fn fold_bool_and() {
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let mut prog = IrProgram {
        functions: vec![IrFunction {
            name: "test".to_owned(),
            params: vec![],
            return_ty: IrType::Bool,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                insts: vec![
                    Inst::ConstBool { dst: v0, value: true },
                    Inst::ConstBool { dst: v1, value: false },
                    Inst::BinOp { dst: v2, op: BinOpKind::And, lhs: v0, rhs: v1 },
                    Inst::Return { value: Some(v2) },
                ],
            }],
            entry: BlockId(0),
            value_types: vec![IrType::Bool, IrType::Bool, IrType::Bool],
        }],
    };
    const_fold(&mut prog);

    let folded = &prog.functions[0].blocks[0].insts[2];
    assert_eq!(*folded, Inst::ConstBool { dst: v2, value: false });
}
