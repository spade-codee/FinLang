//! Shared ISA setup, type mapping, and stdlib symbol tables used by both the
//! JIT and AOT engines.

use std::sync::Arc;

use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::ir::types as clif_types;
use cranelift_codegen::settings::Configurable;
use finlang_ir::IrType;

// ── Public error type ─────────────────────────────────────────────────────────

/// All errors that the codegen crate can produce.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    /// Failed to build or configure the native ISA.
    #[error("ISA setup failed: {0}")]
    IsaSetup(String),

    /// A [`cranelift_module::ModuleError`] wrapped for propagation.
    #[error("Cranelift module error: {0}")]
    ModuleError(String),

    /// A language construct that the backend does not yet support.
    #[error("unsupported construct: {0}")]
    Unsupported(&'static str),

    /// The program did not contain a `__main__` function.
    #[error("no __main__ function in program")]
    MainNotFound,

    /// Internal compiler consistency failure.
    #[error("internal codegen error: {0}")]
    Internal(String),
}

// ── Type mapping ──────────────────────────────────────────────────────────────

/// Convert an [`IrType`] to the corresponding Cranelift IR type.
///
/// * `F64`  → [`cranelift_codegen::ir::types::F64`]
/// * `I64`  → [`cranelift_codegen::ir::types::I64`]
/// * `Bool` → [`cranelift_codegen::ir::types::I8`] — Cranelift has no native
///   boolean; we follow the System V convention of a zero-extended byte.
///
/// # Examples
///
/// ```rust
/// use finlang_codegen::ir_type_to_clif;
/// use finlang_ir::IrType;
/// use cranelift_codegen::ir::types;
///
/// assert_eq!(ir_type_to_clif(IrType::F64),  types::F64);
/// assert_eq!(ir_type_to_clif(IrType::I64),  types::I64);
/// assert_eq!(ir_type_to_clif(IrType::Bool), types::I8);
/// ```
#[must_use]
pub fn ir_type_to_clif(ty: IrType) -> cranelift_codegen::ir::Type {
    match ty {
        IrType::F64  => clif_types::F64,
        IrType::I64  => clif_types::I64,
        IrType::Bool => clif_types::I8,
    }
}

// ── ISA construction ──────────────────────────────────────────────────────────

/// Build a Cranelift [`TargetIsa`] for the host machine with speed-optimised
/// code generation.
///
/// Settings applied:
/// * `opt_level = speed` — generates the fastest code rather than the smallest.
/// * `use_colocated_libcalls = false` — forces absolute addressing for runtime
///   helper calls so the JIT module can patch them correctly.
/// * `is_pic = false` — disables position-independent code; the JIT allocates
///   executable memory in the same address space so PIC is unnecessary overhead.
///
/// # Errors
///
/// Returns [`CodegenError::IsaSetup`] if the host platform is not supported by
/// Cranelift or if a flag name/value is rejected.
pub fn make_isa() -> Result<Arc<dyn TargetIsa>, CodegenError> {
    let mut flag_builder = cranelift_codegen::settings::builder();
    flag_builder
        .set("opt_level", "speed")
        .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;

    let isa_builder = cranelift_native::builder()
        .map_err(|e| CodegenError::IsaSetup(e.to_string()))?;

    isa_builder
        .finish(cranelift_codegen::settings::Flags::new(flag_builder))
        .map_err(|e| CodegenError::IsaSetup(e.to_string()))
}

// ── Stdlib symbol table ───────────────────────────────────────────────────────

/// Return the list of `(symbol_name, function_pointer)` pairs for all
/// `finlang_*` stdlib ABI shims.
///
/// Casting a Rust `fn` item to `*const u8` is a safe coercion — it does not
/// require `unsafe`.  The pointers are used by the JIT engine to register
/// in-process symbols via [`cranelift_jit::JITBuilder::symbol`].
#[must_use]
pub fn stdlib_symbols() -> Vec<(&'static str, *const u8)> {
    use finlang_stdlib as s;
    vec![
        ("finlang_black_scholes",   s::finlang_black_scholes   as *const u8),
        ("finlang_bs_delta",        s::finlang_bs_delta        as *const u8),
        ("finlang_bs_gamma",        s::finlang_bs_gamma        as *const u8),
        ("finlang_bs_vega",         s::finlang_bs_vega         as *const u8),
        ("finlang_bs_theta",        s::finlang_bs_theta        as *const u8),
        ("finlang_bs_rho",          s::finlang_bs_rho          as *const u8),
        ("finlang_implied_vol",     s::finlang_implied_vol     as *const u8),
        ("finlang_bond_price",      s::finlang_bond_price      as *const u8),
        ("finlang_bond_duration",   s::finlang_bond_duration   as *const u8),
        ("finlang_pv01",            s::finlang_pv01            as *const u8),
        ("finlang_discount_factor", s::finlang_discount_factor as *const u8),
        ("finlang_forward_price",   s::finlang_forward_price   as *const u8),
    ]
}

// ── Stdlib signature table ────────────────────────────────────────────────────

/// Look up the signature of a stdlib ABI function by its mangled symbol name.
///
/// Returns `Some((param_types, return_type))` for known stdlib symbols, or
/// `None` for user-defined functions and unknown names.
///
/// The signatures mirror exactly what `finlang-stdlib/src/abi.rs` declares:
/// the `i64` discriminant for `OptionType` is represented as [`IrType::I64`].
///
/// # Examples
///
/// ```rust
/// use finlang_codegen::stdlib_signature;
/// use finlang_ir::IrType;
///
/// let sig = stdlib_signature("finlang_black_scholes").unwrap();
/// assert_eq!(sig.0.len(), 6);          // 5 f64 + 1 i64
/// assert_eq!(sig.1, IrType::F64);      // returns f64
/// ```
#[must_use]
pub fn stdlib_signature(name: &str) -> Option<(&'static [IrType], IrType)> {
    use IrType::{F64, I64};
    match name {
        "finlang_black_scholes"   => Some((&[F64, F64, F64, F64, F64, I64], F64)),
        "finlang_bs_delta"        => Some((&[F64, F64, F64, F64, F64, I64], F64)),
        "finlang_bs_gamma"        => Some((&[F64, F64, F64, F64, F64],      F64)),
        "finlang_bs_vega"         => Some((&[F64, F64, F64, F64, F64],      F64)),
        "finlang_bs_theta"        => Some((&[F64, F64, F64, F64, F64, I64], F64)),
        "finlang_bs_rho"          => Some((&[F64, F64, F64, F64, F64, I64], F64)),
        "finlang_implied_vol"     => Some((&[F64, F64, F64, F64, F64, I64], F64)),
        "finlang_bond_price"      => Some((&[F64, F64, F64, I64],           F64)),
        "finlang_bond_duration"   => Some((&[F64, F64, F64, I64],           F64)),
        "finlang_pv01"            => Some((&[F64, F64, F64, I64],           F64)),
        "finlang_discount_factor" => Some((&[F64, F64],                     F64)),
        "finlang_forward_price"   => Some((&[F64, F64, F64],                F64)),
        _ => None,
    }
}
