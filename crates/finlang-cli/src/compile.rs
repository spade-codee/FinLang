//! `finlang compile <file>` subcommand — AOT to a relocatable object file.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use finlang_codegen::AotEngine;
use target_lexicon::Triple;

use crate::diag::report;
use crate::pipeline::{compile_to_ir, PipelineError};

/// Read `input`, run the pipeline, and write an object file to `output`.
///
/// When `output` is `None`, derives `<stem>.o` from the input file name.
///
/// # Errors
///
/// Returns an I/O error if the input cannot be read or the object file
/// cannot be written.
pub fn compile_file(input: &Path, output: Option<&Path>, quiet: bool) -> Result<i32> {
    let source = std::fs::read_to_string(input)
        .with_context(|| format!("failed to read `{}`", input.display()))?;
    let file_name = input.display().to_string();

    let out = match compile_to_ir(&source) {
        Ok(o) => o,
        Err(e) => {
            report(&file_name, &source, &e, quiet);
            return Ok(1);
        }
    };

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("program");
    let mut engine = match AotEngine::new(Triple::host(), stem) {
        Ok(e) => e,
        Err(e) => {
            report(&file_name, &source, &PipelineError::Codegen(e), quiet);
            return Ok(1);
        }
    };
    let bytes = match engine.compile(&out.program) {
        Ok(b) => b,
        Err(e) => {
            report(&file_name, &source, &PipelineError::Codegen(e), quiet);
            return Ok(1);
        }
    };

    let out_path: PathBuf = match output {
        Some(p) => p.to_owned(),
        None => PathBuf::from(format!("{stem}.o")),
    };
    std::fs::write(&out_path, &bytes)
        .with_context(|| format!("failed to write `{}`", out_path.display()))?;

    if !quiet {
        println!(
            "{} {} ({} bytes)",
            "wrote".green().bold(),
            out_path.display(),
            bytes.len()
        );
    }
    Ok(0)
}
