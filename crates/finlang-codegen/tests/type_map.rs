//! Unit tests for [`finlang_codegen::ir_type_to_clif`].

use cranelift_codegen::ir::types;
use finlang_codegen::ir_type_to_clif;
use finlang_ir::IrType;

#[test]
fn f64_maps_to_clif_f64() {
    assert_eq!(ir_type_to_clif(IrType::F64), types::F64);
}

#[test]
fn i64_maps_to_clif_i64() {
    assert_eq!(ir_type_to_clif(IrType::I64), types::I64);
}

#[test]
fn bool_maps_to_clif_i8() {
    // Booleans are represented as zero-extended bytes (System V convention).
    assert_eq!(ir_type_to_clif(IrType::Bool), types::I8);
}
