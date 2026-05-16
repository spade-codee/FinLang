//! `finlang` — the FinLang command-line driver.
//!
//! Subcommands:
//!
//! | Command                       | Purpose                                  |
//! |-------------------------------|------------------------------------------|
//! | `finlang repl`                | Interactive REPL (default if no args).   |
//! | `finlang run <file>`          | JIT-compile and execute a source file.   |
//! | `finlang compile <file> [-o]` | AOT-compile to a relocatable object.     |
//! | `finlang check <file>`        | Lex + parse + typecheck only.            |
//!
//! Pass `-q` / `--quiet` to suppress informational output.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod compile;
mod diag;
mod pipeline;
mod repl;
mod run;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::diag::report;
use crate::pipeline::compile_to_ir;

/// Top-level command-line interface.
#[derive(Debug, Parser)]
#[command(name = "finlang", version, about = "FinLang compiler & REPL", long_about = None)]
struct Cli {
    /// Suppress informational output; only emit errors and required results.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Optional subcommand.  Defaults to `repl` when omitted.
    #[command(subcommand)]
    command: Option<Command>,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the interactive REPL.
    Repl,
    /// JIT-compile and execute a `.fin` source file.
    Run {
        /// Path to the source file.
        file: PathBuf,
    },
    /// AOT-compile a `.fin` file to a relocatable object file.
    Compile {
        /// Path to the source file.
        file: PathBuf,
        /// Output path (defaults to `<stem>.o`).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Lex, parse, and typecheck a `.fin` file without running it.
    Check {
        /// Path to the source file.
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8> {
    let cli = Cli::parse();
    let cmd = cli.command.unwrap_or(Command::Repl);
    match cmd {
        Command::Repl => {
            repl::run_repl(cli.quiet)?;
            Ok(0)
        }
        Command::Run { file } => {
            let code = run::run_file(&file, cli.quiet)?;
            Ok(u8_from(code))
        }
        Command::Compile { file, output } => {
            let code = compile::compile_file(&file, output.as_deref(), cli.quiet)?;
            Ok(u8_from(code))
        }
        Command::Check { file } => Ok(u8_from(check_file(&file, cli.quiet)?)),
    }
}

/// Run only the front-end up through type checking.  Returns 0 on success
/// and 1 if any parse or type errors were reported.
fn check_file(file: &std::path::Path, quiet: bool) -> Result<i32> {
    let source = std::fs::read_to_string(file)?;
    let file_name = file.display().to_string();
    match compile_to_ir(&source) {
        Ok(_) => {
            if !quiet {
                use colored::Colorize;
                println!("{} {}", "ok:".green().bold(), file.display());
            }
            Ok(0)
        }
        Err(e) => {
            report(&file_name, &source, &e, quiet);
            Ok(1)
        }
    }
}

/// Saturating conversion from `i32` exit code into the `u8` slot used by
/// [`ExitCode`].
fn u8_from(code: i32) -> u8 {
    code.try_into().unwrap_or(1)
}
