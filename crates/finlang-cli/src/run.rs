//! `finlang run <file>` subcommand — JIT compile and execute.

use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use finlang_codegen::{JitEngine, ScalarValue};

use crate::diag::report;
use crate::pipeline::{compile_to_ir, PipelineError};

/// Read `path`, run the full pipeline, JIT compile, and execute `__main__`.
///
/// Returns `Ok(exit_code)` so the caller can propagate failures into the
/// process exit status.
///
/// # Errors
///
/// Returns an I/O error if the source file cannot be read.
pub fn run_file(path: &Path, quiet: bool) -> Result<i32> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;
    let file_name = path.display().to_string();

    let out = match compile_to_ir(&source) {
        Ok(o) => o,
        Err(e) => {
            report(&file_name, &source, &e, quiet);
            return Ok(1);
        }
    };

    let mut engine = match JitEngine::new() {
        Ok(e) => e,
        Err(e) => {
            report(&file_name, &source, &PipelineError::Codegen(e), quiet);
            return Ok(1);
        }
    };
    let program = match engine.compile(&out.program) {
        Ok(p) => p,
        Err(e) => {
            report(&file_name, &source, &PipelineError::Codegen(e), quiet);
            return Ok(1);
        }
    };

    let value = program.run();
    print_value(value, quiet);
    Ok(0)
}

/// Pretty-print a [`ScalarValue`] to stdout.
fn print_value(value: ScalarValue, quiet: bool) {
    match value {
        ScalarValue::F64(v) => {
            if quiet {
                println!("{v}");
            } else {
                println!("{} {:.6}", "result:".green().bold(), v);
            }
        }
        ScalarValue::I64(v) => {
            if quiet {
                println!("{v}");
            } else {
                println!("{} {v}", "result:".green().bold());
            }
        }
        ScalarValue::Bool(v) => {
            if quiet {
                println!("{v}");
            } else {
                println!("{} {v}", "result:".green().bold());
            }
        }
    }
}
