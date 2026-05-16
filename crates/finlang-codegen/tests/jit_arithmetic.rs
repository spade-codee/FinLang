//! JIT tests using hand-constructed IR programs.
//!
//! These tests avoid the full frontend pipeline and directly build minimal
//! [`IrProgram`]s to verify that the JIT engine correctly compiles and executes
//! basic arithmetic and constant-value functions.

mod common;

use finlang_codegen::{JitEngine, ScalarValue};
use finlang_ir::{BasicBlock, BlockId, IrFunction, IrProgram, IrType, Inst, ValueId};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a zero-parameter [`IrProgram`] with a single `__main__` function that
/// contains the provided instructions.  The last instruction must be a `Return`.
fn make_program(return_ty: IrType, insts: Vec<Inst>) -> IrProgram {
    // Collect the type of each defined value from the instructions.
    let mut value_types: Vec<IrType> = Vec::new();
    for inst in &insts {
        if let Some(dst) = inst.dst() {
            // Extend value_types so index `dst.0` is valid.
            while value_types.len() <= dst.0 as usize {
                value_types.push(IrType::F64); // placeholder
            }
        }
    }
    // Second pass: fill in the correct types.
    for inst in &insts {
        match inst {
            Inst::ConstF64 { dst, .. } => value_types[dst.0 as usize] = IrType::F64,
            Inst::ConstI64 { dst, .. } => value_types[dst.0 as usize] = IrType::I64,
            Inst::ConstBool { dst, .. } => value_types[dst.0 as usize] = IrType::Bool,
            Inst::BinOp { dst, .. } => value_types[dst.0 as usize] = return_ty,
            Inst::UnaryOp { dst, .. } => value_types[dst.0 as usize] = return_ty,
            Inst::CastIntToFloat { dst, .. } => value_types[dst.0 as usize] = IrType::F64,
            Inst::CastFloatToInt { dst, .. } => value_types[dst.0 as usize] = IrType::I64,
            Inst::Call { dst: Some(d), .. } => {
                value_types[d.0 as usize] = return_ty;
            }
            Inst::Call { dst: None, .. } => {}
            _ => {}
        }
    }

    let bb = BasicBlock { id: BlockId(0), insts };
    let func = IrFunction {
        name: "__main__".to_owned(),
        params: vec![],
        return_ty,
        blocks: vec![bb],
        entry: BlockId(0),
        value_types,
    };
    IrProgram { functions: vec![func] }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn jit_f64_add() {
    // 5.0 + 3.0 = 8.0
    let insts = vec![
        Inst::ConstF64 { dst: ValueId(0), value: 5.0 },
        Inst::ConstF64 { dst: ValueId(1), value: 3.0 },
        Inst::BinOp {
            dst: ValueId(2),
            op: finlang_parser::ast::BinOpKind::Add,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
        Inst::Return { value: Some(ValueId(2)) },
    ];
    let prog = make_program(IrType::F64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    assert_eq!(result, ScalarValue::F64(8.0), "5.0 + 3.0 should be 8.0");
}

#[test]
fn jit_f64_mul() {
    // 0.1 * 2.0 = 0.2
    let insts = vec![
        Inst::ConstF64 { dst: ValueId(0), value: 0.1 },
        Inst::ConstF64 { dst: ValueId(1), value: 2.0 },
        Inst::BinOp {
            dst: ValueId(2),
            op: finlang_parser::ast::BinOpKind::Mul,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
        Inst::Return { value: Some(ValueId(2)) },
    ];
    let prog = make_program(IrType::F64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    if let ScalarValue::F64(v) = result {
        assert!(
            (v - 0.2_f64).abs() < 1e-15,
            "0.1 * 2.0 should be ~0.2, got {v}"
        );
    } else {
        panic!("expected ScalarValue::F64, got {result:?}");
    }
}

#[test]
fn jit_i64_return() {
    // Return the integer 7.
    let insts = vec![
        Inst::ConstI64 { dst: ValueId(0), value: 7 },
        Inst::Return { value: Some(ValueId(0)) },
    ];
    let prog = make_program(IrType::I64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    assert_eq!(result, ScalarValue::I64(7), "should return 7");
}

#[test]
fn jit_f64_sub_and_div() {
    // (10.0 - 4.0) / 2.0 = 3.0
    let insts = vec![
        Inst::ConstF64 { dst: ValueId(0), value: 10.0 },
        Inst::ConstF64 { dst: ValueId(1), value: 4.0 },
        Inst::BinOp {
            dst: ValueId(2),
            op: finlang_parser::ast::BinOpKind::Sub,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
        Inst::ConstF64 { dst: ValueId(3), value: 2.0 },
        Inst::BinOp {
            dst: ValueId(4),
            op: finlang_parser::ast::BinOpKind::Div,
            lhs: ValueId(2),
            rhs: ValueId(3),
        },
        Inst::Return { value: Some(ValueId(4)) },
    ];
    let prog = make_program(IrType::F64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    assert_eq!(result, ScalarValue::F64(3.0), "(10-4)/2 should be 3.0");
}

#[test]
fn jit_cast_int_to_float() {
    // (i64)3 cast to f64 = 3.0
    let insts = vec![
        Inst::ConstI64 { dst: ValueId(0), value: 3 },
        Inst::CastIntToFloat { dst: ValueId(1), src: ValueId(0) },
        Inst::Return { value: Some(ValueId(1)) },
    ];
    let prog = make_program(IrType::F64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    assert_eq!(result, ScalarValue::F64(3.0), "cast i64→f64 should be 3.0");
}

#[test]
fn jit_i64_add() {
    // 100 + 23 = 123
    let insts = vec![
        Inst::ConstI64 { dst: ValueId(0), value: 100 },
        Inst::ConstI64 { dst: ValueId(1), value: 23 },
        Inst::BinOp {
            dst: ValueId(2),
            op: finlang_parser::ast::BinOpKind::Add,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
        Inst::Return { value: Some(ValueId(2)) },
    ];
    let prog = make_program(IrType::I64, insts);
    let mut engine = JitEngine::new().expect("JitEngine::new");
    let jit = engine.compile(&prog).expect("compile");
    let result = jit.run();
    assert_eq!(result, ScalarValue::I64(123));
}
