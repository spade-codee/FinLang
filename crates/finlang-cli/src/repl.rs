//! Interactive REPL for `finlang repl`.
//!
//! Built on [`rustyline`] with persistent history at `$HOME/.finlang_history`
//! (or `%USERPROFILE%\.finlang_history` on Windows).  Multiline input is
//! detected by tracking the net balance of `{` `(` versus `}` `)`, ignoring
//! characters inside string literals.  A trailing backslash also forces
//! continuation.
//!
//! Commands beginning with `:` are dispatched to dot-command handlers:
//!
//! | Command          | Behaviour                                            |
//! |------------------|------------------------------------------------------|
//! | `:type <expr>`   | Parse + typecheck, print the inferred [`FinType`].   |
//! | `:ir <expr>`     | Pipeline through DCE, print the IR.                  |
//! | `:asm <expr>`    | Placeholder — Cranelift asm dump is v0.1+.           |
//! | `:bench <expr>`  | JIT-compile, run for ~1s, print throughput.          |
//! | `:load <path>`   | Read a file and submit its contents as input.        |
//! | `:help`          | Print command reference and example programs.        |
//! | `:quit`/`:exit`  | Leave the REPL.                                      |

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;
use finlang_codegen::{JitEngine, ScalarValue};
use finlang_ir::IrProgram;
use finlang_parser::parse_str;
use finlang_types::check;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::diag::report;
use crate::pipeline::{compile_to_ir, PipelineError};

/// Enter the interactive REPL.  Returns when the user issues `:quit` or sends
/// EOF.  `Ctrl-C` cancels the current line but does not exit.
///
/// # Errors
///
/// Returns an error only if rustyline fails to initialise.
pub fn run_repl(quiet: bool) -> Result<()> {
    if !quiet {
        print_banner();
    }

    let mut editor = DefaultEditor::new()?;
    let history_path = history_path();
    if let Some(ref p) = history_path {
        let _ = editor.load_history(p);
    }

    loop {
        let prompt = format!("{} ", "finlang>".green().bold());
        let mut buffer = match editor.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("{} {e}", "readline error:".red().bold());
                break;
            }
        };

        // Accumulate continuation lines while the buffer is unbalanced.
        while needs_continuation(&buffer) {
            let cont_prompt = format!("{} ", "    ...>".yellow().bold());
            match editor.readline(&cont_prompt) {
                Ok(line) => {
                    buffer.push('\n');
                    buffer.push_str(&line);
                }
                Err(ReadlineError::Interrupted) => {
                    buffer.clear();
                    break;
                }
                Err(_) => break,
            }
        }

        let input = buffer.trim();
        if input.is_empty() {
            continue;
        }

        let _ = editor.add_history_entry(input);
        if let Some(ref p) = history_path {
            let _ = editor.save_history(p);
        }

        if let Some(rest) = input.strip_prefix(':') {
            if handle_dot_command(rest, quiet) {
                break;
            }
        } else {
            evaluate(input, quiet);
        }
    }

    Ok(())
}

/// Print the welcome banner and a short tip.
fn print_banner() {
    println!(
        "{} v{}  — type {} for commands, {} to exit",
        "FinLang REPL".bright_cyan().bold(),
        env!("CARGO_PKG_VERSION"),
        ":help".bright_white().bold(),
        ":quit".bright_white().bold()
    );
}

/// Compute the history file location, preferring `$HOME` then `$USERPROFILE`.
fn history_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".finlang_history"))
}

/// Dispatch a `:command` line.  Returns `true` to signal the loop to exit.
fn handle_dot_command(rest: &str, quiet: bool) -> bool {
    let (cmd, arg) = split_cmd_arg(rest);
    match cmd {
        "quit" | "exit" | "q" => return true,
        "help" | "h" => print_help(),
        "type" => cmd_type(arg, quiet),
        "ir" => cmd_ir(arg, quiet),
        "asm" => cmd_asm(),
        "bench" => cmd_bench(arg, quiet),
        "load" => cmd_load(arg, quiet),
        other => {
            eprintln!(
                "{} unknown command `:{other}` (try {})",
                "error:".red().bold(),
                ":help".bright_white().bold()
            );
        }
    }
    false
}

/// Split a dot-command line into `(verb, argument_or_empty)`.
fn split_cmd_arg(line: &str) -> (&str, &str) {
    match line.find(char::is_whitespace) {
        Some(i) => (&line[..i], line[i..].trim()),
        None => (line.trim(), ""),
    }
}

/// `:help` — print command reference.
fn print_help() {
    println!("{}", "Commands:".bright_cyan().bold());
    println!("  :type <expr>   show the inferred type of <expr>");
    println!("  :ir <expr>     show the optimised SSA IR for <expr>");
    println!("  :asm <expr>    show JIT-compiled assembly (not implemented in v0.1)");
    println!("  :bench <expr>  benchmark <expr> for ~1 second");
    println!("  :load <path>   read <path> and evaluate its contents");
    println!("  :help          show this message");
    println!("  :quit          exit the REPL");
    println!();
    println!("{}", "Example:".bright_cyan().bold());
    println!("  finlang> let s: price = 100.0 as price");
    println!("  finlang> let k: price = 95.0 as price");
    println!("  finlang> s - k");
}

/// `:type <expr>` — parse and typecheck, then print the type of the final expression.
fn cmd_type(arg: &str, quiet: bool) {
    if arg.is_empty() {
        eprintln!("{} `:type` requires an expression", "error:".red().bold());
        return;
    }
    let parsed = parse_str(arg);
    if !parsed.errors.is_empty() {
        report("<repl>", arg, &PipelineError::Parse(parsed.errors), quiet);
        return;
    }
    let types = check(&parsed.items);
    if !types.errors.is_empty() {
        report("<repl>", arg, &PipelineError::Type(types.errors), quiet);
        return;
    }
    // Print the type of the last top-level item via `expr_types` lookup using
    // the item's span.  Items implement `span()` indirectly; fall back to a
    // generic message if we can't determine it.
    if let Some(last) = parsed.items.last() {
        let span = item_span(last);
        if let Some(ty) = span.and_then(|s| types.expr_types.get(&s)) {
            println!("{} {}", "type:".bright_cyan().bold(), ty);
        } else {
            println!("{} (no expression result)", "type:".bright_cyan().bold());
        }
    }
}

/// Extract a `Span` from a top-level [`finlang_parser::ast::Item`] when possible.
fn item_span(item: &finlang_parser::ast::Item) -> Option<finlang_lexer::Span> {
    use finlang_parser::ast::Item;
    match item {
        Item::LetDecl { span, .. }
        | Item::FnDef { span, .. }
        | Item::PortfolioDef { span, .. } => Some(*span),
        Item::ExprItem(_, span) => Some(*span),
    }
}

/// `:ir <expr>` — run the optimisation pipeline and print the IR.
fn cmd_ir(arg: &str, quiet: bool) {
    if arg.is_empty() {
        eprintln!("{} `:ir` requires an expression", "error:".red().bold());
        return;
    }
    match compile_to_ir(arg) {
        Ok(out) => print!("{}", out.program),
        Err(e) => report("<repl>", arg, &e, quiet),
    }
}

/// `:asm` — currently a stub.
fn cmd_asm() {
    eprintln!(
        "{} `:asm` requires the optional `asm-dump` build (not implemented in v0.1)",
        "note:".yellow().bold()
    );
}

/// `:bench <expr>` — JIT compile once, then run for ~1 second.
fn cmd_bench(arg: &str, quiet: bool) {
    if arg.is_empty() {
        eprintln!("{} `:bench` requires an expression", "error:".red().bold());
        return;
    }
    let program = match compile_to_ir(arg) {
        Ok(o) => o.program,
        Err(e) => {
            report("<repl>", arg, &e, quiet);
            return;
        }
    };
    let compiled = match jit_compile(&program) {
        Ok(c) => c,
        Err(e) => {
            report("<repl>", arg, &PipelineError::Codegen(e), quiet);
            return;
        }
    };

    // Warm-up.
    let _ = compiled.run();

    let target = std::time::Duration::from_secs(1);
    let mut iters: u64 = 0;
    let start = Instant::now();
    while start.elapsed() < target {
        // Batch to amortise the elapsed check.
        for _ in 0..1024 {
            std::hint::black_box(compiled.run());
        }
        iters += 1024;
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as f64 / iters as f64;
    let ops = iters as f64 / elapsed.as_secs_f64();
    println!(
        "{} {iters} iters in {:.3}s — {:.1} ns/op, {:.2e} ops/sec",
        "bench:".bright_cyan().bold(),
        elapsed.as_secs_f64(),
        ns_per,
        ops
    );
}

/// `:load <path>` — read a file and evaluate its contents.
fn cmd_load(arg: &str, quiet: bool) {
    if arg.is_empty() {
        eprintln!("{} `:load` requires a path", "error:".red().bold());
        return;
    }
    match std::fs::read_to_string(arg) {
        Ok(src) => evaluate(&src, quiet),
        Err(e) => eprintln!("{} {e}", "error:".red().bold()),
    }
}

/// Evaluate a free-form input as a complete top-level program.
fn evaluate(input: &str, quiet: bool) {
    let program = match compile_to_ir(input) {
        Ok(o) => o.program,
        Err(e) => {
            report("<repl>", input, &e, quiet);
            return;
        }
    };
    let compiled = match jit_compile(&program) {
        Ok(c) => c,
        Err(e) => {
            report("<repl>", input, &PipelineError::Codegen(e), quiet);
            return;
        }
    };
    let value = compiled.run();
    print_repl_value(value);
}

/// Build a [`JitEngine`] and compile the supplied program.
fn jit_compile(
    program: &IrProgram,
) -> Result<finlang_codegen::JitProgram, finlang_codegen::CodegenError> {
    let mut engine = JitEngine::new()?;
    engine.compile(program)
}

/// Print a [`ScalarValue`] using the REPL's `=> result` style.
fn print_repl_value(value: ScalarValue) {
    let arrow = "=>".bright_cyan().bold();
    match value {
        ScalarValue::F64(v) => println!("{arrow} {v:.6}"),
        ScalarValue::I64(v) => println!("{arrow} {v}"),
        ScalarValue::Bool(v) => println!("{arrow} {v}"),
    }
}

/// Decide whether `buffer` is incomplete and needs more lines.
///
/// Tracks net `{ ( - } )` and a trailing backslash.  Characters inside
/// string literals are not counted.
pub(crate) fn needs_continuation(buffer: &str) -> bool {
    if buffer.trim_end().ends_with('\\') {
        return true;
    }
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut prev_backslash = false;
    for c in buffer.chars() {
        if in_string {
            match c {
                '\\' if !prev_backslash => prev_backslash = true,
                '"' if !prev_backslash => in_string = false,
                _ => prev_backslash = false,
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            _ => {}
        }
    }
    depth > 0
}

#[cfg(test)]
mod tests {
    use super::needs_continuation;

    #[test]
    fn balanced_block_is_complete() {
        assert!(!needs_continuation("let x = { 1 + 2 }"));
    }

    #[test]
    fn unbalanced_open_brace_needs_more() {
        assert!(needs_continuation("let x = {"));
    }

    #[test]
    fn open_paren_needs_more() {
        assert!(needs_continuation("let f(x: int) = ("));
    }

    #[test]
    fn brace_inside_string_is_ignored() {
        assert!(!needs_continuation("let s = \"hello { world\""));
    }

    #[test]
    fn trailing_backslash_forces_continuation() {
        assert!(needs_continuation("let x = 1 + \\"));
    }

    #[test]
    fn escaped_quote_inside_string_does_not_close_it() {
        // String stays open across the whole input, so braces inside don't count
        // and the net brace depth is 0.
        assert!(!needs_continuation("let s = \"a\\\"b{c\""));
    }
}
