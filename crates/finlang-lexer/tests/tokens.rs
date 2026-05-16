//! Unit tests for the finlang-lexer token set.
//!
//! Each test is narrow and targets a single aspect of the lexer so failures
//! localise quickly.

use finlang_lexer::{tokenize, Span, Token};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return the token nodes from `tokenize`, dropping the trailing `Eof`.
fn lex(source: &str) -> Vec<Token> {
    let mut v: Vec<Token> = tokenize(source).into_iter().map(|s| s.node).collect();
    assert_eq!(v.last(), Some(&Token::Eof), "last token must be Eof");
    v.pop();
    v
}

/// Single-token helper: assert `source` lexes to exactly one non-Eof token.
fn single(source: &str) -> Token {
    let tokens = lex(source);
    assert_eq!(tokens.len(), 1, "expected 1 token, got {tokens:?}");
    tokens.into_iter().next().unwrap()
}

// ── Keywords ──────────────────────────────────────────────────────────────────

#[test]
fn keyword_let() {
    assert_eq!(single("let"), Token::Let);
}

#[test]
fn keyword_fn() {
    assert_eq!(single("fn"), Token::Fn);
}

#[test]
fn keyword_portfolio() {
    assert_eq!(single("portfolio"), Token::Portfolio);
}

#[test]
fn keyword_long() {
    assert_eq!(single("long"), Token::Long);
}

#[test]
fn keyword_short() {
    assert_eq!(single("short"), Token::Short);
}

#[test]
fn keyword_for() {
    assert_eq!(single("for"), Token::For);
}

#[test]
fn keyword_in() {
    assert_eq!(single("in"), Token::In);
}

#[test]
fn keyword_if() {
    assert_eq!(single("if"), Token::If);
}

#[test]
fn keyword_else() {
    assert_eq!(single("else"), Token::Else);
}

#[test]
fn keyword_as() {
    assert_eq!(single("as"), Token::As);
}

#[test]
fn keyword_at() {
    assert_eq!(single("at"), Token::At);
}

#[test]
fn keyword_return() {
    assert_eq!(single("return"), Token::Return);
}

#[test]
fn keyword_true() {
    assert_eq!(single("true"), Token::True);
}

#[test]
fn keyword_false() {
    assert_eq!(single("false"), Token::False);
}

// ── Type keywords ─────────────────────────────────────────────────────────────

#[test]
fn kw_price() {
    assert_eq!(single("price"), Token::Price);
}

#[test]
fn kw_rate() {
    assert_eq!(single("rate"), Token::Rate);
}

#[test]
fn kw_notional() {
    assert_eq!(single("notional"), Token::Notional);
}

#[test]
fn kw_date() {
    assert_eq!(single("date"), Token::Date);
}

#[test]
fn kw_years() {
    assert_eq!(single("years"), Token::Years);
}

#[test]
fn kw_basis_points() {
    assert_eq!(single("basis_points"), Token::BasisPoints);
}

#[test]
fn kw_bool() {
    assert_eq!(single("bool"), Token::Bool);
}

#[test]
fn kw_int() {
    assert_eq!(single("int"), Token::Int);
}

// ── Built-in enum values ──────────────────────────────────────────────────────

#[test]
fn enum_call() {
    assert_eq!(single("Call"), Token::Call);
}

#[test]
fn enum_put() {
    assert_eq!(single("Put"), Token::Put);
}

// ── Operators — single-character ─────────────────────────────────────────────

#[test]
fn op_plus() {
    assert_eq!(single("+"), Token::Plus);
}

#[test]
fn op_minus() {
    assert_eq!(single("-"), Token::Minus);
}

#[test]
fn op_star() {
    assert_eq!(single("*"), Token::Star);
}

#[test]
fn op_slash() {
    assert_eq!(single("/"), Token::Slash);
}

#[test]
fn op_percent() {
    assert_eq!(single("%"), Token::Percent);
}

#[test]
fn op_lt() {
    assert_eq!(single("<"), Token::Lt);
}

#[test]
fn op_gt() {
    assert_eq!(single(">"), Token::Gt);
}

#[test]
fn op_bang() {
    assert_eq!(single("!"), Token::Bang);
}

#[test]
fn op_eq() {
    assert_eq!(single("="), Token::Eq);
}

// ── Operators — multi-character ───────────────────────────────────────────────

#[test]
fn op_eqeq() {
    assert_eq!(single("=="), Token::EqEq);
}

#[test]
fn op_noteq() {
    assert_eq!(single("!="), Token::NotEq);
}

#[test]
fn op_lteq() {
    assert_eq!(single("<="), Token::LtEq);
}

#[test]
fn op_gteq() {
    assert_eq!(single(">="), Token::GtEq);
}

#[test]
fn op_andand() {
    assert_eq!(single("&&"), Token::AndAnd);
}

#[test]
fn op_oror() {
    assert_eq!(single("||"), Token::OrOr);
}

#[test]
fn op_arrow() {
    assert_eq!(single("->"), Token::Arrow);
}

// ── Delimiters ────────────────────────────────────────────────────────────────

#[test]
fn delimiters_all() {
    let tokens = lex("(){}[],;:");
    assert_eq!(
        tokens,
        vec![
            Token::LParen,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
            Token::Comma,
            Token::Semi,
            Token::Colon,
        ]
    );
}

// ── Numeric literals ──────────────────────────────────────────────────────────

#[test]
fn int_plain() {
    assert_eq!(single("42"), Token::IntLit(42));
}

#[test]
fn int_underscores() {
    assert_eq!(single("1_000_000"), Token::IntLit(1_000_000));
}

#[test]
fn int_zero() {
    assert_eq!(single("0"), Token::IntLit(0));
}

#[test]
fn float_plain() {
    // Use a value that does not trigger clippy::approx_constant (not pi).
    assert_eq!(single("2.71"), Token::FloatLit(2.71));
}

#[test]
fn float_underscores() {
    assert_eq!(single("1_000.50"), Token::FloatLit(1000.50));
}

#[test]
fn float_leading_zero() {
    assert_eq!(single("0.05"), Token::FloatLit(0.05));
}

// ── Identifiers ───────────────────────────────────────────────────────────────

#[test]
fn ident_simple() {
    assert_eq!(single("foo"), Token::Ident("foo".to_owned()));
}

#[test]
fn ident_underscore_prefix() {
    assert_eq!(single("_hidden"), Token::Ident("_hidden".to_owned()));
}

#[test]
fn ident_with_digits() {
    assert_eq!(single("x1"), Token::Ident("x1".to_owned()));
}

/// `letter` must be an identifier, NOT `let` + `ter`.
#[test]
fn ident_starts_with_kw_let() {
    assert_eq!(single("letter"), Token::Ident("letter".to_owned()));
}

/// `intersect` must be an identifier, NOT `in` + `tersect`.
#[test]
fn ident_starts_with_kw_in() {
    assert_eq!(single("intersect"), Token::Ident("intersect".to_owned()));
}

/// `priced` must be an identifier, NOT `price` + `d`.
#[test]
fn ident_starts_with_kw_price() {
    assert_eq!(single("priced"), Token::Ident("priced".to_owned()));
}

/// `format` must be an identifier, NOT `for` + something.
#[test]
fn ident_starts_with_kw_for() {
    assert_eq!(single("format"), Token::Ident("format".to_owned()));
}

/// `returning` must be an identifier.
#[test]
fn ident_starts_with_kw_return() {
    assert_eq!(single("returning"), Token::Ident("returning".to_owned()));
}

// ── String literals ───────────────────────────────────────────────────────────

#[test]
fn string_simple() {
    assert_eq!(
        single(r#""hello""#),
        Token::StringLit("hello".to_owned())
    );
}

#[test]
fn string_escape_newline() {
    assert_eq!(
        single(r#""line1\nline2""#),
        Token::StringLit("line1\nline2".to_owned())
    );
}

#[test]
fn string_escape_quote() {
    assert_eq!(
        single(r#""say \"hi\"""#),
        Token::StringLit(r#"say "hi""#.to_owned())
    );
}

#[test]
fn string_escape_backslash() {
    assert_eq!(
        single(r#""a\\b""#),
        Token::StringLit("a\\b".to_owned())
    );
}

#[test]
fn string_bad_escape() {
    let tok = single(r#""bad \k escape""#);
    assert!(
        matches!(tok, Token::LexError(ref m) if m.contains("unknown escape")),
        "expected LexError for bad escape, got {tok:?}"
    );
}

#[test]
fn string_unterminated() {
    let tok = single(r#""not closed"#);
    assert!(
        matches!(tok, Token::LexError(ref m) if m.contains("unterminated")),
        "expected LexError for unterminated string, got {tok:?}"
    );
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[test]
fn comment_skipped() {
    let tokens = lex("// this is a comment\nlet");
    assert_eq!(tokens, vec![Token::Let]);
}

#[test]
fn comment_inline_skipped() {
    let tokens = lex("42 // answer");
    assert_eq!(tokens, vec![Token::IntLit(42)]);
}

// ── Error tokens ──────────────────────────────────────────────────────────────

#[test]
fn unrecognised_at_sign() {
    let tok = single("@");
    assert!(
        matches!(tok, Token::LexError(ref m) if m.contains('@')),
        "expected LexError mentioning '@', got {tok:?}"
    );
}

#[test]
fn unrecognised_hash() {
    let tok = single("#");
    assert!(
        matches!(tok, Token::LexError(ref m) if m.contains('#')),
        "expected LexError mentioning '#', got {tok:?}"
    );
}

// ── Eof guarantee ────────────────────────────────────────────────────────────

#[test]
fn empty_source_ends_with_eof() {
    let tokens = tokenize("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].node, Token::Eof);
}

#[test]
fn non_empty_source_ends_with_eof() {
    let tokens = tokenize("let x = 1");
    assert_eq!(tokens.last().unwrap().node, Token::Eof);
}

// ── Span correctness ──────────────────────────────────────────────────────────
//
// Source:  "let x = 5"
//           0123456789
//           ^^^       -> let   [0,3)
//               ^     -> x     [4,5)
//                 ^   -> =     [6,7)
//                   ^ -> 5     [8,9)

#[test]
fn spans_let_x_eq_5() {
    let spanned = tokenize("let x = 5");
    // drop Eof
    let without_eof = &spanned[..spanned.len() - 1];

    assert_eq!(without_eof[0].span, Span::new(0, 3)); // let
    assert_eq!(without_eof[1].span, Span::new(4, 5)); // x
    assert_eq!(without_eof[2].span, Span::new(6, 7)); // =
    assert_eq!(without_eof[3].span, Span::new(8, 9)); // 5
}

#[test]
fn span_merge() {
    let a = Span::new(0, 3);
    let b = Span::new(5, 9);
    assert_eq!(a.merge(b), Span::new(0, 9));
}

#[test]
fn eof_span_is_source_len() {
    let src = "let";
    let tokens = tokenize(src);
    let eof = tokens.last().unwrap();
    assert_eq!(eof.node, Token::Eof);
    assert_eq!(eof.span.start, src.len());
    assert_eq!(eof.span.end, src.len());
}

// ── Display impl ─────────────────────────────────────────────────────────────

#[test]
fn display_keywords() {
    assert_eq!(Token::Let.to_string(), "let");
    assert_eq!(Token::Portfolio.to_string(), "portfolio");
    assert_eq!(Token::BasisPoints.to_string(), "basis_points");
}

#[test]
fn display_operators() {
    assert_eq!(Token::Arrow.to_string(), "->");
    assert_eq!(Token::EqEq.to_string(), "==");
    assert_eq!(Token::LtEq.to_string(), "<=");
}

#[test]
fn display_eof() {
    assert_eq!(Token::Eof.to_string(), "<eof>");
}

// ── Mixed expression ──────────────────────────────────────────────────────────

#[test]
fn mixed_expression() {
    let tokens = lex("let val: rate = 0.05 + 1_000");
    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::Ident("val".to_owned()),
            Token::Colon,
            Token::Rate,
            Token::Eq,
            Token::FloatLit(0.05),
            Token::Plus,
            Token::IntLit(1000),
        ]
    );
}
