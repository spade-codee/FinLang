//! Parse error types and codespan-based diagnostic rendering.
//!
//! All variants carry a [`Span`] so that [`render_error`] can emit
//! precise source-level carets.

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{BufferedStandardStream, ColorChoice, NoColor};
use finlang_lexer::Span;
use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Every kind of error the parser can produce.
///
/// Variants are ordered from most-common to least-common to aid pattern-match
/// readability in the type checker and IDE integrations.
#[derive(Debug, Clone, PartialEq, Error)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ParseError {
    /// A token was found where a different one was expected.
    #[error("expected {expected}, found '{found}' at {span}")]
    UnexpectedToken {
        /// Human-readable description of what was expected.
        expected: String,
        /// The token that was actually present.
        found: String,
        /// Source location of the offending token.
        span: Span,
    },

    /// The token stream ended before the grammar rule could complete.
    #[error("unexpected end of file: expected {expected} at {span}")]
    UnexpectedEof {
        /// Human-readable description of what was expected at EOF.
        expected: String,
        /// The span of the EOF token.
        span: Span,
    },

    /// A comparison operator was chained without parentheses, e.g. `a < b < c`.
    ///
    /// This is a deliberate grammar restriction: comparison operators in
    /// FinLang are non-associative.  Use `(a < b) && (b < c)` instead.
    #[error("comparison operators cannot be chained — use parentheses or && to combine ({span})")]
    ChainedComparison {
        /// Span covering the two comparison operators.
        span: Span,
    },

    /// A token appeared where a type annotation was expected.
    #[error("invalid type annotation at {span}: {msg}")]
    InvalidTypeAnnotation {
        /// Source location.
        span: Span,
        /// Diagnostic message.
        msg: String,
    },

    /// A [`Token::LexError`] was encountered in the token stream.
    ///
    /// The lexer never fails; it encodes bad input as `LexError` tokens.
    /// The parser surfaces them here so all diagnostics share a single
    /// reporting path.
    #[error("lex error at {span}: {msg}")]
    LexErrorBubbled {
        /// The lexer's own diagnostic message.
        msg: String,
        /// Source location of the bad token.
        span: Span,
    },
}

impl ParseError {
    /// The source span associated with this error.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            ParseError::UnexpectedToken { span, .. }
            | ParseError::UnexpectedEof { span, .. }
            | ParseError::ChainedComparison { span }
            | ParseError::InvalidTypeAnnotation { span, .. }
            | ParseError::LexErrorBubbled { span, .. } => *span,
        }
    }

    /// Build a [`codespan_reporting`] `Diagnostic` for this error.
    fn to_diagnostic(&self) -> Diagnostic<()> {
        let span = self.span();
        let range = span.start..span.end;

        match self {
            ParseError::UnexpectedToken { expected, found, .. } => {
                Diagnostic::error()
                    .with_message(format!("expected {expected}, found '{found}'"))
                    .with_labels(vec![
                        Label::primary((), range).with_message("unexpected token here"),
                    ])
            }
            ParseError::UnexpectedEof { expected, .. } => {
                Diagnostic::error()
                    .with_message(format!("unexpected end of file: expected {expected}"))
                    .with_labels(vec![
                        Label::primary((), range).with_message("file ends here"),
                    ])
            }
            ParseError::ChainedComparison { .. } => {
                Diagnostic::error()
                    .with_message(
                        "comparison operators cannot be chained — \
                         use parentheses or && to combine",
                    )
                    .with_labels(vec![
                        Label::primary((), range).with_message("chained comparison here"),
                    ])
                    .with_notes(vec!["try `(a < b) && (b < c)` instead".to_owned()])
            }
            ParseError::InvalidTypeAnnotation { msg, .. } => {
                Diagnostic::error()
                    .with_message("invalid type annotation")
                    .with_labels(vec![
                        Label::primary((), range).with_message(msg.as_str()),
                    ])
            }
            ParseError::LexErrorBubbled { msg, .. } => {
                Diagnostic::error()
                    .with_message(format!("lex error: {msg}"))
                    .with_labels(vec![
                        Label::primary((), range).with_message("bad token here"),
                    ])
            }
        }
    }
}

/// Render `error` against `source` using codespan-reporting without ANSI
/// colour codes, returning the result as a plain `String`.
///
/// Useful in tests where you need to assert on the exact rendered output.
///
/// # Arguments
///
/// * `file_name` — displayed in the diagnostic header (e.g. `"option_pricing.fin"`).
/// * `source`    — the original source text (used to print the relevant line).
/// * `error`     — the error to render.
///
/// # Examples
///
/// ```rust
/// use finlang_parser::{parse_str, error::render_error};
///
/// let result = parse_str("let x = ;");
/// if let Some(err) = result.errors.first() {
///     let rendered = render_error("test.fin", "let x = ;", err);
///     assert!(rendered.contains("expected"));
/// }
/// ```
pub fn render_error(file_name: &str, source: &str, error: &ParseError) -> String {
    let file = SimpleFile::new(file_name, source);
    let diag = error.to_diagnostic();
    let config = term::Config::default();

    let mut buf = Vec::<u8>::new();
    {
        let mut writer = NoColor::new(&mut buf);
        term::emit(&mut writer, &config, &file, &diag)
            .unwrap_or_default();
    }
    String::from_utf8(buf).unwrap_or_default()
}

/// Render `error` against `source` with ANSI colour codes on the current
/// terminal's standard error stream.
///
/// Intended for the REPL and CLI. Colour is auto-detected via
/// [`ColorChoice::Auto`].
///
/// # Arguments
///
/// * `file_name` — displayed in the diagnostic header.
/// * `source`    — the original source text.
/// * `error`     — the error to render.
pub fn render_error_colored(file_name: &str, source: &str, error: &ParseError) {
    let file = SimpleFile::new(file_name, source);
    let diag = error.to_diagnostic();
    let config = term::Config::default();
    let mut stream = BufferedStandardStream::stderr(ColorChoice::Auto);
    let _ = term::emit(&mut stream, &config, &file, &diag);
}
