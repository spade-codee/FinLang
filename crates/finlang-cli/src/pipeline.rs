//! Shared compiler pipeline: source → AST → typed AST → SSA IR → optimised IR.
//!
//! Used by every subcommand (`check`, `run`, `compile`) and by the REPL.
//! The pipeline is intentionally a free function that returns rich error
//! structures so each subcommand can decide its own presentation policy.

use finlang_codegen::CodegenError;
use finlang_ir::{const_fold, dce, lower, validate_ssa, IrProgram, LowerError};
use finlang_ir::validate::ValidationError;
use finlang_parser::{parse_str, ParseError};
use finlang_types::{check, TypeError};

/// Result of running the front-end pipeline up to and including IR optimisation.
pub struct PipelineOutput {
    /// The fully-optimised, SSA-validated IR program.
    pub program: IrProgram,
}

/// Every failure mode the pipeline can produce, in source-order layers.
#[derive(Debug)]
pub enum PipelineError {
    /// One or more parse errors.
    Parse(Vec<ParseError>),
    /// One or more type errors.
    Type(Vec<TypeError>),
    /// AST → IR lowering failed.
    Lower(LowerError),
    /// SSA validation failed (compiler bug).
    Validate(ValidationError),
    /// Cranelift code generation failed (only surfaced by callers that JIT/AOT).
    Codegen(CodegenError),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Parse(errs) => write!(f, "{} parse error(s)", errs.len()),
            PipelineError::Type(errs) => write!(f, "{} type error(s)", errs.len()),
            PipelineError::Lower(e) => write!(f, "lowering failed: {e}"),
            PipelineError::Validate(e) => write!(f, "ssa invariant violated: {e}"),
            PipelineError::Codegen(e) => write!(f, "code generation failed: {e}"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Run the front-end pipeline from source string through optimised IR.
///
/// Stops at the first failing layer; downstream layers are not attempted.
///
/// # Errors
///
/// Returns the first [`PipelineError`] encountered.
pub fn compile_to_ir(source: &str) -> Result<PipelineOutput, PipelineError> {
    let parsed = parse_str(source);
    if !parsed.errors.is_empty() {
        return Err(PipelineError::Parse(parsed.errors));
    }
    let types = check(&parsed.items);
    if !types.errors.is_empty() {
        return Err(PipelineError::Type(types.errors));
    }
    let mut program = lower(&parsed.items, &types).map_err(PipelineError::Lower)?;
    const_fold(&mut program);
    dce(&mut program);
    validate_ssa(&program).map_err(PipelineError::Validate)?;
    Ok(PipelineOutput { program })
}
