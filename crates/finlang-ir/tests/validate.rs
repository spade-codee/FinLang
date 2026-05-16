//! Unit tests for `validate_ssa`.
//!
//! Each test hand-constructs a deliberately malformed IR program and asserts
//! that the validator catches the violation with the expected error variant.

use finlang_ir::ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};
use finlang_ir::validate::ValidationError;
use finlang_ir::validate_ssa;
use finlang_parser::ast::BinOpKind;

fn one_function(name: &str, blocks: Vec<BasicBlock>, value_types: Vec<IrType>) -> IrProgram {
    IrProgram {
        functions: vec![IrFunction {
            name: name.to_owned(),
            params: vec![],
            return_ty: IrType::F64,
            entry: blocks[0].id,
            blocks,
            value_types,
        }],
    }
}

// ── Well-formed program passes ────────────────────────────────────────────────

#[test]
fn validate_accepts_well_formed_program() {
    let v0 = ValueId(0);
    let prog = one_function(
        "ok",
        vec![BasicBlock {
            id: BlockId(0),
            insts: vec![
                Inst::ConstF64 { dst: v0, value: 1.0 },
                Inst::Return { value: Some(v0) },
            ],
        }],
        vec![IrType::F64],
    );
    assert!(validate_ssa(&prog).is_ok());
}

// ── Use of undefined value ────────────────────────────────────────────────────

#[test]
fn validate_catches_undefined_value() {
    // Return references v99, which is never defined.
    let v99 = ValueId(99);
    let prog = one_function(
        "bad_undef",
        vec![BasicBlock {
            id: BlockId(0),
            insts: vec![Inst::Return { value: Some(v99) }],
        }],
        vec![IrType::F64; 100],
    );

    match validate_ssa(&prog) {
        Err(ValidationError::UndefinedValue { value, function }) => {
            assert_eq!(value, v99);
            assert_eq!(function, "bad_undef");
        }
        other => panic!("expected UndefinedValue, got {other:?}"),
    }
}

#[test]
fn validate_catches_undefined_operand_in_binop() {
    let v0 = ValueId(0);
    let v_missing = ValueId(42);
    let v_dst = ValueId(1);
    let prog = one_function(
        "bad_binop",
        vec![BasicBlock {
            id: BlockId(0),
            insts: vec![
                Inst::ConstF64 { dst: v0, value: 1.0 },
                Inst::BinOp { dst: v_dst, op: BinOpKind::Add, lhs: v0, rhs: v_missing },
                Inst::Return { value: Some(v_dst) },
            ],
        }],
        vec![IrType::F64; 50],
    );

    match validate_ssa(&prog) {
        Err(ValidationError::UndefinedValue { value, .. }) => {
            assert_eq!(value, v_missing);
        }
        other => panic!("expected UndefinedValue for v_missing, got {other:?}"),
    }
}

// ── Missing terminator ────────────────────────────────────────────────────────

#[test]
fn validate_catches_missing_terminator() {
    // Block ends with a non-terminator instruction.
    let v0 = ValueId(0);
    let prog = one_function(
        "bad_term",
        vec![BasicBlock {
            id: BlockId(0),
            insts: vec![Inst::ConstF64 { dst: v0, value: 1.0 }],
        }],
        vec![IrType::F64],
    );

    match validate_ssa(&prog) {
        Err(ValidationError::MissingTerminator { block, function }) => {
            assert_eq!(block, BlockId(0));
            assert_eq!(function, "bad_term");
        }
        other => panic!("expected MissingTerminator, got {other:?}"),
    }
}

#[test]
fn validate_catches_empty_block() {
    let prog = one_function(
        "empty_block",
        vec![BasicBlock { id: BlockId(0), insts: vec![] }],
        vec![],
    );
    match validate_ssa(&prog) {
        Err(ValidationError::MissingTerminator { block, .. }) => {
            assert_eq!(block, BlockId(0));
        }
        other => panic!("expected MissingTerminator for empty block, got {other:?}"),
    }
}

// ── Phi referencing nonexistent block ─────────────────────────────────────────

#[test]
fn validate_catches_phi_with_nonexistent_predecessor() {
    let v0 = ValueId(0);
    let v_phi = ValueId(1);
    // Single block bb0; phi claims its incoming value comes from bb99, which
    // doesn't exist.
    let prog = one_function(
        "bad_phi",
        vec![BasicBlock {
            id: BlockId(0),
            insts: vec![
                Inst::ConstF64 { dst: v0, value: 1.0 },
                Inst::Phi {
                    dst: v_phi,
                    incoming: vec![(v0, BlockId(99))],
                },
                Inst::Return { value: Some(v_phi) },
            ],
        }],
        vec![IrType::F64; 2],
    );

    match validate_ssa(&prog) {
        Err(ValidationError::PhiReferencesNonexistentBlock {
            phi_dst,
            missing_block,
            function,
        }) => {
            assert_eq!(phi_dst, v_phi);
            assert_eq!(missing_block, BlockId(99));
            assert_eq!(function, "bad_phi");
        }
        other => panic!("expected PhiReferencesNonexistentBlock, got {other:?}"),
    }
}
