//! Diagnostic rendering for parser and type-checker errors.
//!
//! Type errors are rendered via [`finlang_types::render_error`], which already
//! emits a codespan-style report.  Parse errors do not have a dedicated
//! renderer, so this module produces a small `caret`-style diagnostic with
//! line and column information derived from the source byte offsets.

use colored::Colorize;
use finlang_lexer::Span;
use finlang_parser::ParseError;
use finlang_types::render_error;

use crate::pipeline::PipelineError;

/// Print every error in `err` to stderr, formatted for `file_name` / `source`.
///
/// `quiet` collapses each error to a single line.  When `false`, parse errors
/// get a caret span and type errors get the full codespan rendering.
pub fn report(file_name: &str, source: &str, err: &PipelineError, quiet: bool) {
    match err {
        PipelineError::Parse(errs) => {
            for e in errs {
                if quiet {
                    eprintln!("{} {}", "error:".red().bold(), e);
                } else {
                    eprint!("{}", render_parse_error(file_name, source, e));
                }
            }
        }
        PipelineError::Type(errs) => {
            for e in errs {
                if quiet {
                    eprintln!("{} {}", "error:".red().bold(), e);
                } else {
                    eprint!("{}", render_error(file_name, source, e));
                    eprintln!();
                }
            }
        }
        PipelineError::Lower(e) => {
            eprintln!("{} {}", "internal error:".red().bold(), e);
        }
        PipelineError::Validate(e) => {
            eprintln!("{} {}", "internal error:".red().bold(), e);
        }
        PipelineError::Codegen(e) => {
            eprintln!("{} {}", "internal error:".red().bold(), e);
        }
    }
}

/// Render a single [`ParseError`] with a caret span.
///
/// Output shape:
/// ```text
/// error: unexpected token `]`
///   --> file.fin:3:14
///   |
/// 3 | let x = foo(]
///   |              ^
/// ```
fn render_parse_error(file_name: &str, source: &str, err: &ParseError) -> String {
    let span = err.span();
    let (line, col, line_text) = locate(source, span);
    let mut out = String::new();
    out.push_str(&format!("{} {}\n", "error:".red().bold(), err));
    out.push_str(&format!(
        "  {} {}:{}:{}\n",
        "-->".bright_blue().bold(),
        file_name,
        line,
        col
    ));
    out.push_str(&format!("  {}\n", "|".bright_blue().bold()));
    out.push_str(&format!(
        "{:>3} {} {}\n",
        line.to_string().bright_blue().bold(),
        "|".bright_blue().bold(),
        line_text
    ));
    let span_len = span.end.saturating_sub(span.start).max(1);
    let caret = "^".repeat(span_len);
    out.push_str(&format!(
        "  {} {}{}\n",
        "|".bright_blue().bold(),
        " ".repeat(col.saturating_sub(1)),
        caret.red().bold()
    ));
    out
}

/// Compute 1-based `(line, column, line_text)` for the start of `span`.
fn locate(source: &str, span: Span) -> (usize, usize, &str) {
    let clamped = span.start.min(source.len());
    let prefix = &source[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = clamped - line_start + 1;
    let line_end = source[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(source.len());
    (line, col, &source[line_start..line_end])
}

