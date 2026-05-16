//! FinLang lexer.
//!
//! Tokenises FinLang source code into a flat [`Vec`] of [`Spanned<Token>`].
//! This is the foundational layer of the compiler pipeline; it has no
//! dependency on any other FinLang crate.
//!
//! # Quick start
//!
//! ```rust
//! use finlang_lexer::{tokenize, Token};
//!
//! let tokens = tokenize("let x: int = 42");
//! assert_eq!(tokens[0].node, Token::Let);
//! assert_eq!(tokens[1].node, Token::Ident("x".to_owned()));
//! ```
//!
//! # Error handling
//!
//! [`tokenize`] **always** succeeds.  Unrecognised input is represented as
//! [`Token::LexError`] entries in the returned vector rather than as a
//! `Result`.  The vector is always terminated by a [`Token::Eof`] entry.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use logos::Logos;

// ── Public types ─────────────────────────────────────────────────────────────

/// A byte-offset range `[start, end)` into the original source string.
///
/// Offsets are counted in UTF-8 bytes, matching the slice indices you would
/// use on a `&str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    /// Construct a new span from explicit byte offsets.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Return the smallest span that covers both `self` and `other`.
    #[must_use]
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A value together with its source [`Span`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Spanned<T> {
    /// The wrapped value.
    pub node: T,
    /// Byte span of this item in the source.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Wrap `node` with a [`Span`].
    #[must_use]
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

// ── Token ────────────────────────────────────────────────────────────────────

/// Every token kind produced by [`tokenize`].
///
/// Variants follow the grammar spec exactly so the parser can `match` them
/// without any additional mapping layer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Token {
    // --- Keywords ---
    /// `let`
    Let,
    /// `fn`
    Fn,
    /// `portfolio`
    Portfolio,
    /// `long`
    Long,
    /// `short`
    Short,
    /// `for`
    For,
    /// `in`
    In,
    /// `if`
    If,
    /// `else`
    Else,
    /// `as`
    As,
    /// `at`
    At,
    /// `return`
    Return,
    /// `true`
    True,
    /// `false`
    False,

    // --- Type keywords ---
    /// `price`
    Price,
    /// `rate`
    Rate,
    /// `notional`
    Notional,
    /// `date`
    Date,
    /// `years`
    Years,
    /// `basis_points`
    BasisPoints,
    /// `bool`
    Bool,
    /// `int`
    Int,

    // --- Built-in enum values ---
    /// `Call`
    Call,
    /// `Put`
    Put,

    // --- Literals ---
    /// An identifier: `[a-zA-Z_][a-zA-Z0-9_]*` (not a keyword).
    Ident(String),
    /// A decimal integer literal (underscores stripped): `[0-9][0-9_]*`.
    IntLit(i64),
    /// A decimal float literal (underscores stripped): `[0-9][0-9_]*\.[0-9][0-9_]*`.
    FloatLit(f64),
    /// A string literal with escapes resolved.
    StringLit(String),

    // --- Operators ---
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    LtEq,
    /// `>=`
    GtEq,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `!`
    Bang,
    /// `=`
    Eq,
    /// `->`
    Arrow,

    // --- Delimiters ---
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `;`
    Semi,

    // --- Special ---
    /// End of file; always the last token in the vector returned by [`tokenize`].
    Eof,
    /// A recoverable lexer error with a human-readable diagnostic.
    LexError(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Let => write!(f, "let"),
            Token::Fn => write!(f, "fn"),
            Token::Portfolio => write!(f, "portfolio"),
            Token::Long => write!(f, "long"),
            Token::Short => write!(f, "short"),
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::As => write!(f, "as"),
            Token::At => write!(f, "at"),
            Token::Return => write!(f, "return"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Price => write!(f, "price"),
            Token::Rate => write!(f, "rate"),
            Token::Notional => write!(f, "notional"),
            Token::Date => write!(f, "date"),
            Token::Years => write!(f, "years"),
            Token::BasisPoints => write!(f, "basis_points"),
            Token::Bool => write!(f, "bool"),
            Token::Int => write!(f, "int"),
            Token::Call => write!(f, "Call"),
            Token::Put => write!(f, "Put"),
            Token::Ident(s) => write!(f, "{s}"),
            Token::IntLit(n) => write!(f, "{n}"),
            Token::FloatLit(n) => write!(f, "{n}"),
            Token::StringLit(s) => write!(f, "\"{s}\""),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::EqEq => write!(f, "=="),
            Token::NotEq => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::LtEq => write!(f, "<="),
            Token::GtEq => write!(f, ">="),
            Token::AndAnd => write!(f, "&&"),
            Token::OrOr => write!(f, "||"),
            Token::Bang => write!(f, "!"),
            Token::Eq => write!(f, "="),
            Token::Arrow => write!(f, "->"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Semi => write!(f, ";"),
            Token::Eof => write!(f, "<eof>"),
            Token::LexError(msg) => write!(f, "<error: {msg}>"),
        }
    }
}

// ── Internal logos token ──────────────────────────────────────────────────────

/// Internal logos-derived token.  This is not public; callers see [`Token`].
///
/// Logos drives the regex-based scanning pass.  A thin post-processing step
/// in [`tokenize`] converts every `LogosToken` into a `Token`, resolving
/// string escapes and numeric parsing at that point.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")] // whitespace
#[logos(skip r"//[^\n]*")]   // line comments
enum LogosToken {
    // ---- Multi-char operators (must come before single-char prefixes) --------
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("->")]
    Arrow,

    // ---- Single-char operators -----------------------------------------------
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("!")]
    Bang,
    #[token("=")]
    Eq,

    // ---- Delimiters ----------------------------------------------------------
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,

    // ---- Float (must come before Int to avoid consuming the integer prefix) --
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*")]
    Float,

    // ---- Integer -------------------------------------------------------------
    #[regex(r"[0-9][0-9_]*")]
    Int,

    // ---- String literal (raw slice; escapes resolved in post-pass) ----------
    #[regex(r#""([^"\\]|\\.)*""#)]
    StringComplete,

    /// An unterminated string: starts with `"` and never closes before EOL/EOF.
    #[regex(r#""([^"\\]|\\.)*"#)]
    StringUnterminated,

    // ---- Keywords and identifiers -------------------------------------------
    //
    // Logos resolves priority by declaration order when multiple patterns match
    // the same slice, *and* longer matches win over shorter ones.  Because
    // keywords are `#[token]` (exact) and identifiers are `#[regex]`, logos
    // naturally gives an exact-token match higher priority than a regex match
    // of the same length.  So `let` → `Let`, but `letter` → `Ident` because
    // `letter` does not equal any keyword token string.

    // Keywords
    #[token("let")]
    Let,
    #[token("fn")]
    Fn,
    #[token("portfolio")]
    Portfolio,
    #[token("long")]
    Long,
    #[token("short")]
    Short,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("as")]
    As,
    #[token("at")]
    At,
    #[token("return")]
    Return,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // Type keywords
    #[token("price")]
    Price,
    #[token("rate")]
    Rate,
    #[token("notional")]
    Notional,
    #[token("date")]
    Date,
    #[token("years")]
    Years,
    #[token("basis_points")]
    BasisPoints,
    #[token("bool")]
    Bool,
    #[token("int")]
    IntKw,

    // Built-in enum values
    #[token("Call")]
    Call,
    #[token("Put")]
    Put,

    // Identifier (lowest priority among word-like patterns)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}

// ── String escape resolution ──────────────────────────────────────────────────

/// Resolve escape sequences in a raw quoted string slice (including the
/// surrounding `"` characters).
///
/// Returns `Ok(String)` on success, or an error message string describing the
/// first bad escape encountered.
fn resolve_string_escapes(raw: &str) -> Result<String, String> {
    // Strip surrounding quotes.
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some(other) => {
                return Err(format!("unknown escape sequence '\\{other}'"));
            }
            None => {
                // Trailing backslash — shouldn't occur with our regex but be safe.
                return Err("unexpected end of string after '\\'".to_owned());
            }
        }
    }
    Ok(out)
}

// ── Numeric parsing ───────────────────────────────────────────────────────────

/// Strip underscore separators from a numeric literal slice, then parse as
/// `i64`.
fn parse_int(raw: &str) -> Result<i64, String> {
    let stripped: String = raw.chars().filter(|&c| c != '_').collect();
    stripped
        .parse::<i64>()
        .map_err(|_| format!("integer literal '{raw}' overflows i64"))
}

/// Strip underscore separators from a numeric literal slice, then parse as
/// `f64`.
fn parse_float(raw: &str) -> Result<f64, String> {
    let stripped: String = raw.chars().filter(|&c| c != '_').collect();
    let v: f64 = stripped
        .parse::<f64>()
        .map_err(|_| format!("float literal '{raw}' is not a valid f64"))?;
    if v.is_infinite() {
        return Err(format!("float literal '{raw}' overflows f64"));
    }
    Ok(v)
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Tokenise `source` into a flat vector of spanned tokens.
///
/// This function **always succeeds**.  Any unrecognised input or malformed
/// literal is encoded as a [`Token::LexError`] entry rather than being
/// returned as a `Result` error.  The returned vector is always terminated by
/// a [`Token::Eof`] entry whose span points one past the end of the source.
///
/// # Examples
///
/// ```rust
/// use finlang_lexer::{tokenize, Token, Span};
///
/// let ts = tokenize("let x = 1");
/// assert_eq!(ts[0].node, Token::Let);
/// assert_eq!(ts[0].span, Span::new(0, 3));
/// assert_eq!(ts[3].node, Token::IntLit(1));
/// assert!(matches!(ts.last().unwrap().node, Token::Eof));
/// ```
pub fn tokenize(source: &str) -> Vec<Spanned<Token>> {
    let mut output: Vec<Spanned<Token>> = Vec::new();
    let lexer = LogosToken::lexer(source);

    for (result, span) in lexer.spanned() {
        let pub_span = Span::new(span.start, span.end);
        let raw = &source[span.clone()];

        let token = match result {
            Ok(LogosToken::EqEq) => Token::EqEq,
            Ok(LogosToken::NotEq) => Token::NotEq,
            Ok(LogosToken::LtEq) => Token::LtEq,
            Ok(LogosToken::GtEq) => Token::GtEq,
            Ok(LogosToken::AndAnd) => Token::AndAnd,
            Ok(LogosToken::OrOr) => Token::OrOr,
            Ok(LogosToken::Arrow) => Token::Arrow,
            Ok(LogosToken::Plus) => Token::Plus,
            Ok(LogosToken::Minus) => Token::Minus,
            Ok(LogosToken::Star) => Token::Star,
            Ok(LogosToken::Slash) => Token::Slash,
            Ok(LogosToken::Percent) => Token::Percent,
            Ok(LogosToken::Lt) => Token::Lt,
            Ok(LogosToken::Gt) => Token::Gt,
            Ok(LogosToken::Bang) => Token::Bang,
            Ok(LogosToken::Eq) => Token::Eq,
            Ok(LogosToken::LParen) => Token::LParen,
            Ok(LogosToken::RParen) => Token::RParen,
            Ok(LogosToken::LBrace) => Token::LBrace,
            Ok(LogosToken::RBrace) => Token::RBrace,
            Ok(LogosToken::LBracket) => Token::LBracket,
            Ok(LogosToken::RBracket) => Token::RBracket,
            Ok(LogosToken::Comma) => Token::Comma,
            Ok(LogosToken::Colon) => Token::Colon,
            Ok(LogosToken::Semi) => Token::Semi,
            Ok(LogosToken::Let) => Token::Let,
            Ok(LogosToken::Fn) => Token::Fn,
            Ok(LogosToken::Portfolio) => Token::Portfolio,
            Ok(LogosToken::Long) => Token::Long,
            Ok(LogosToken::Short) => Token::Short,
            Ok(LogosToken::For) => Token::For,
            Ok(LogosToken::In) => Token::In,
            Ok(LogosToken::If) => Token::If,
            Ok(LogosToken::Else) => Token::Else,
            Ok(LogosToken::As) => Token::As,
            Ok(LogosToken::At) => Token::At,
            Ok(LogosToken::Return) => Token::Return,
            Ok(LogosToken::True) => Token::True,
            Ok(LogosToken::False) => Token::False,
            Ok(LogosToken::Price) => Token::Price,
            Ok(LogosToken::Rate) => Token::Rate,
            Ok(LogosToken::Notional) => Token::Notional,
            Ok(LogosToken::Date) => Token::Date,
            Ok(LogosToken::Years) => Token::Years,
            Ok(LogosToken::BasisPoints) => Token::BasisPoints,
            Ok(LogosToken::Bool) => Token::Bool,
            Ok(LogosToken::IntKw) => Token::Int,
            Ok(LogosToken::Call) => Token::Call,
            Ok(LogosToken::Put) => Token::Put,
            Ok(LogosToken::Ident) => Token::Ident(raw.to_owned()),
            Ok(LogosToken::Int) => match parse_int(raw) {
                Ok(n) => Token::IntLit(n),
                Err(msg) => Token::LexError(msg),
            },
            Ok(LogosToken::Float) => match parse_float(raw) {
                Ok(n) => Token::FloatLit(n),
                Err(msg) => Token::LexError(msg),
            },
            Ok(LogosToken::StringComplete) => match resolve_string_escapes(raw) {
                Ok(s) => Token::StringLit(s),
                Err(msg) => Token::LexError(msg),
            },
            Ok(LogosToken::StringUnterminated) => Token::LexError(format!(
                "unterminated string literal starting at byte {}",
                span.start
            )),
            Err(()) => {
                // Logos emits () for unrecognised input.  We extract the
                // offending character for a diagnostic; the span is always at
                // least one byte wide.
                let bad_char = source[span.clone()]
                    .chars()
                    .next()
                    .unwrap_or('\x00');
                Token::LexError(format!(
                    "unexpected character '{bad_char}' at byte {}",
                    span.start
                ))
            }
        };
        output.push(Spanned::new(token, pub_span));
    }

    let eof_offset = source.len();
    output.push(Spanned::new(Token::Eof, Span::new(eof_offset, eof_offset)));
    output
}
