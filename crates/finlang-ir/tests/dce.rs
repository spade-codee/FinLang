//! Unit tests for the dead-code-elimination pass.
//!
//! Programs are hand-constructed directly in IR so the tests don't drag in
//! the lowering pipeline.

use finlang_ir::ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};
use finlang_ir::opt::dce;
use finlang_ir::validate_ssa;
use finlang_parser::ast::BinOpKind;

/// Build a single-block IR program. `value_types` is filled with `F64` for
/// every dst found in the instructions (sufficient for these tests).
fn make_program(insts: Vec<Inst>) -> IrProgram {
    let max_vid = insts
        .iter()
        .filter_map(|i| i.dst())
        .map(|v| v.0)
        .max()
        .unwrap_or(0);
    let value_types = vec![IrType::F64; (max_vid + 1) as usize];

    IrProgram {
        functions: vec![IrFunction {
            name: "test".to_owned(),
            params: vec![],
            return_ty: IrType::F64,
            blocks: vec![BasicBlock { id: BlockId(0), insts }],
            entry: BlockId(0),
            value_types,
        }],
    }
}

fn block_inst_count(prog: &IrProgram) -> usize {
    prog.functions[0].blocks[0].insts.len()
}

// ── 1. Unused let — DCE removes its defining instruction ──────────────────────

#[test]
fn dce_removes_unused_let() {
    // v0 = 1.0     (used by return)
    // v1 = 2.0     (unused)
    // return v0
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let mut prog = make_program(vec![
        Inst::ConstF64 { dst: v0, value: 1.0 },
        Inst::ConstF64 { dst: v1, value: 2.0 },
        Inst::Return { value: Some(v0) },
    ]);
    assert_eq!(block_inst_count(&prog), 3);

    dce(&mut prog);

    // v1 should be gone; v0 and return survive.
    assert_eq!(block_inst_count(&prog), 2);
    let insts = &prog.functions[0].blocks[0].insts;
    assert!(matches!(insts[0], Inst::ConstF64 { dst, .. } if dst == v0));
    assert!(matches!(insts[1], Inst::Return { .. }));
    assert!(validate_ssa(&prog).is_ok());
}

// ── 2. Value used by a call stays live ────────────────────────────────────────

#[test]
fn dce_keeps_values_used_by_calls() {
    // v0 = 1.0
    // v1 = 2.0
    // v2 = call f(v0, v1)   ; call's return value unused
    // v3 = 99.0             ; this one is unused, return won't reach it
    // return v3
    //
    // Even though v2's return value is unused, the call itself stays (side
    // effects). v0 and v1 stay because the call uses them. v3 stays because
    // return uses it.
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let v3 = ValueId(3);
    let mut prog = make_program(vec![
        Inst::ConstF64 { dst: v0, value: 1.0 },
        Inst::ConstF64 { dst: v1, value: 2.0 },
        Inst::Call {
            dst: Some(v2),
            callee: "finlang_discount_factor".to_owned(),
            args: vec![v0, v1],
        },
        Inst::ConstF64 { dst: v3, value: 99.0 },
        Inst::Return { value: Some(v3) },
    ]);

    dce(&mut prog);

    // Nothing should be removed — the call holds v0 and v1 live, return holds v3.
    let insts = &prog.functions[0].blocks[0].insts;
    assert!(matches!(insts[0], Inst::ConstF64 { dst, .. } if dst == v0));
    assert!(matches!(insts[1], Inst::ConstF64 { dst, .. } if dst == v1));
    assert!(matches!(insts[2], Inst::Call { .. }));
    assert!(matches!(insts[3], Inst::ConstF64 { dst, .. } if dst == v3));
    assert!(matches!(insts[4], Inst::Return { .. }));
    assert!(validate_ssa(&prog).is_ok());
}

// ── 3. Cascading dead-code elimination ────────────────────────────────────────

#[test]
fn dce_cascades_through_chain() {
    // v0 = 1.0
    // v1 = v0 + v0     (uses v0)
    // v2 = v1 * v1     (uses v1)
    // v3 = 5.0         (the only thing return uses)
    // return v3
    //
    // None of v0/v1/v2 are needed; cascading DCE should remove all three.
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let v3 = ValueId(3);
    let mut prog = make_program(vec![
        Inst::ConstF64 { dst: v0, value: 1.0 },
        Inst::BinOp { dst: v1, op: BinOpKind::Add, lhs: v0, rhs: v0 },
        Inst::BinOp { dst: v2, op: BinOpKind::Mul, lhs: v1, rhs: v1 },
        Inst::ConstF64 { dst: v3, value: 5.0 },
        Inst::Return { value: Some(v3) },
    ]);
    assert_eq!(block_inst_count(&prog), 5);

    dce(&mut prog);

    // Only v3 and the return should remain.
    let insts = &prog.functions[0].blocks[0].insts;
    assert_eq!(insts.len(), 2);
    assert!(matches!(insts[0], Inst::ConstF64 { dst, value } if dst == v3 && value == 5.0));
    assert!(matches!(insts[1], Inst::Return { value: Some(v) } if v == v3));
    assert!(validate_ssa(&prog).is_ok());
}

// ── 4. DCE preserves SSA validity (this is the debug_assert!) ─────────────────

#[test]
fn dce_preserves_ssa_validity_on_realistic_program() {
    // A realistic shape: several lets, a binop chain, and a return.
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let v2 = ValueId(2);
    let v3 = ValueId(3);
    let v4 = ValueId(4);
    let v5 = ValueId(5);
    let mut prog = make_program(vec![
        Inst::ConstF64 { dst: v0, value: 10.0 },
        Inst::ConstF64 { dst: v1, value: 20.0 },
        Inst::BinOp { dst: v2, op: BinOpKind::Add, lhs: v0, rhs: v1 },
        Inst::ConstF64 { dst: v3, value: 7.0 }, // unused
        Inst::ConstF64 { dst: v4, value: 3.0 }, // unused
        Inst::BinOp { dst: v5, op: BinOpKind::Mul, lhs: v2, rhs: v0 },
        Inst::Return { value: Some(v5) },
    ]);

    dce(&mut prog);

    assert!(
        validate_ssa(&prog).is_ok(),
        "DCE must preserve SSA invariants"
    );
    // v3 and v4 are gone; the others remain.
    let remaining_dsts: Vec<_> = prog.functions[0].blocks[0]
        .insts
        .iter()
        .filter_map(|i| i.dst())
        .collect();
    assert!(remaining_dsts.contains(&v0));
    assert!(remaining_dsts.contains(&v1));
    assert!(remaining_dsts.contains(&v2));
    assert!(remaining_dsts.contains(&v5));
    assert!(!remaining_dsts.contains(&v3));
    assert!(!remaining_dsts.contains(&v4));
}
