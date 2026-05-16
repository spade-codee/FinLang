//! FinLang parser.
//!
//! Hand-written recursive-descent parser that consumes the token stream
//! produced by [`finlang_lexer::tokenize`] and builds a typed AST.
//!
//! # Design
//!
//! The parser is a single-pass, zero-copy recursive descent over a
//! `&[Spanned<Token>]` slice held in [`Parser`].  Each grammar production maps
//! to one `parse_*` method; operator precedence is encoded as a chain of
//! mutually-recursive functions (one per level), not a Pratt table, so every
//! precedence boundary is explicit and auditable by eye.
//!
//! # Error recovery
//!
//! A single bad statement does **not** abort the file.  The parser collects
//! every [`ParseError`] into [`ParseResult::errors`] and resyncs at `;`,
//! `}`, or the next top-level keyword before continuing.
//!
//! # Quick start
//!
//! ```rust
//! use finlang_parser::{parse_str, ast::Item};
//!
//! let result = parse_str("let x: int = 42");
//! assert!(result.errors.is_empty());
//! assert_eq!(result.items.len(), 1);
//! assert!(matches!(result.items[0], Item::LetDecl { .. }));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod ast;
pub mod error;

pub use ast::*;
pub use error::ParseError;

use finlang_lexer::{Span, Spanned, Token};

// ── Public result type ────────────────────────────────────────────────────────

/// The combined output of a parse run.
///
/// Both fields may be non-empty simultaneously: the parser recovers from
/// errors and continues producing items wherever it can.
#[derive(Debug)]
pub struct ParseResult {
    /// The top-level items successfully constructed.
    pub items: Vec<ast::Item>,
    /// Every error encountered, in source order.
    pub errors: Vec<ParseError>,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a pre-tokenised stream into a [`ParseResult`].
///
/// The `tokens` slice must end with a [`Token::Eof`] entry (as
/// [`finlang_lexer::tokenize`] guarantees).
///
/// # Examples
///
/// ```rust
/// use finlang_lexer::tokenize;
/// use finlang_parser::parse;
///
/// let tokens = tokenize("let x: int = 1");
/// let result = parse(&tokens);
/// assert!(result.errors.is_empty());
/// ```
pub fn parse(tokens: &[Spanned<Token>]) -> ParseResult {
    let mut p = Parser::new(tokens);
    p.parse_file()
}

/// Lex `source` with [`finlang_lexer::tokenize`] and then parse it.
///
/// This is the convenience entry point for the REPL, tests, and the CLI.
///
/// # Examples
///
/// ```rust
/// use finlang_parser::parse_str;
///
/// let result = parse_str("let y: rate = 0.05");
/// assert!(result.errors.is_empty());
/// assert_eq!(result.items.len(), 1);
/// ```
pub fn parse_str(source: &str) -> ParseResult {
    let tokens = finlang_lexer::tokenize(source);
    parse(&tokens)
}

// ── Parser state ──────────────────────────────────────────────────────────────

/// Recursive-descent parser state.
///
/// Holds a shared reference to the token slice and a cursor index.  No heap
/// allocations are made by the parser itself beyond the AST nodes it
/// constructs.
struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    cursor: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Spanned<Token>]) -> Self {
        Self {
            tokens,
            cursor: 0,
            errors: Vec::new(),
        }
    }

    // ── Token access ─────────────────────────────────────────────────────────

    /// Peek at the current token without consuming it.
    fn peek(&self) -> &Token {
        &self.tokens[self.cursor].node
    }

    /// The span of the current token.
    fn current_span(&self) -> Span {
        self.tokens[self.cursor].span
    }

    /// Peek at the token `offset` positions ahead (0 = current).
    fn peek_ahead(&self, offset: usize) -> &Token {
        let idx = (self.cursor + offset).min(self.tokens.len() - 1);
        &self.tokens[idx].node
    }

    /// Advance the cursor and return the consumed token and its span.
    fn advance(&mut self) -> &Spanned<Token> {
        let t = &self.tokens[self.cursor];
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
        t
    }

    /// Advance if the current token matches `expected`, returning its span.
    /// Otherwise record an [`ParseError::UnexpectedToken`] and return `None`.
    fn expect(&mut self, expected: &Token, expected_desc: &str) -> Option<Span> {
        if self.peek() == expected {
            let sp = self.current_span();
            self.advance();
            Some(sp)
        } else {
            let span = self.current_span();
            let found = self.peek().to_string();
            self.errors.push(ParseError::UnexpectedToken {
                expected: expected_desc.to_owned(),
                found,
                span,
            });
            None
        }
    }

    /// Return `true` and advance if the current token equals `tok`.
    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Is the current token `Token::Eof`?
    fn is_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    // ── Identifier helpers ────────────────────────────────────────────────────

    /// Try to extract the string name of the current token if it can serve as
    /// an identifier in a name position.
    ///
    /// In addition to bare `Ident` tokens, a limited set of keywords are
    /// allowed as user-chosen names (e.g. `portfolio` as a variable name in
    /// `bond_portfolio.fin`).  Type-keywords are also accepted here so that
    /// a user can write `let date = …` without a parse error; the type checker
    /// enforces shadowing rules later.
    fn token_as_name(tok: &Token) -> Option<String> {
        match tok {
            Token::Ident(s) => Some(s.clone()),
            // Allow keywords that realistically appear as variable names in
            // FinLang examples.  All non-structural keywords are fair game;
            // structural ones (fn, let, for, return, if, else, portfolio,
            // long, short, in) remain reserved so the grammar stays LL(1).
            Token::Price => Some("price".to_owned()),
            Token::Rate => Some("rate".to_owned()),
            Token::Notional => Some("notional".to_owned()),
            Token::Date => Some("date".to_owned()),
            Token::Years => Some("years".to_owned()),
            Token::BasisPoints => Some("basis_points".to_owned()),
            Token::Bool => Some("bool".to_owned()),
            Token::Int => Some("int".to_owned()),
            Token::Portfolio => Some("portfolio".to_owned()),
            Token::At => Some("at".to_owned()),
            Token::As => Some("as".to_owned()),
            _ => None,
        }
    }

    /// Consume a token that can serve as an identifier (see [`Self::token_as_name`])
    /// and return `(name, span)`.  Records an [`ParseError::UnexpectedToken`]
    /// and returns `None` if the current token is not name-like.
    fn expect_ident(&mut self, ctx: &str) -> Option<(String, Span)> {
        if let Some(name) = Self::token_as_name(self.peek()) {
            let sp = self.current_span();
            self.advance();
            Some((name, sp))
        } else {
            let span = self.current_span();
            let found = self.peek().to_string();
            self.errors.push(ParseError::UnexpectedToken {
                expected: ctx.to_owned(),
                found,
                span,
            });
            None
        }
    }

    // ── Skip / resync ─────────────────────────────────────────────────────────

    /// Skip tokens until we reach a statement boundary or `}`.
    ///
    /// Used to recover inside a block after a bad statement.
    fn skip_to_stmt_boundary(&mut self) {
        loop {
            match self.peek() {
                Token::Semi | Token::RBrace | Token::Eof => break,
                Token::Fn | Token::Portfolio | Token::Let | Token::Return | Token::For => break,
                _ => {
                    self.advance();
                }
            }
        }
        // consume the `;` if that's what we stopped at
        self.eat(&Token::Semi);
    }

    /// Skip tokens until we are at the start of the next top-level item or EOF.
    ///
    /// Used to recover after a bad top-level item.
    fn skip_to_item_boundary(&mut self) {
        loop {
            match self.peek() {
                Token::Eof | Token::Fn | Token::Portfolio | Token::Let => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ── Optional statement terminators ───────────────────────────────────────

    /// Consume zero or more semicolons (newlines are already stripped by the
    /// lexer, so only `;` acts as an explicit separator here).
    fn eat_semis(&mut self) {
        while self.eat(&Token::Semi) {}
    }

    // ── Top-level file parse ──────────────────────────────────────────────────

    fn parse_file(&mut self) -> ParseResult {
        let mut items: Vec<ast::Item> = Vec::new();
        self.eat_semis();

        while !self.is_eof() {
            match self.peek() {
                Token::Fn => {
                    match self.parse_fn_def() {
                        Some(item) => items.push(item),
                        None => self.skip_to_item_boundary(),
                    }
                }
                Token::Portfolio => {
                    // Disambiguate: `portfolio NAME { … }` is a PortfolioDef;
                    // a bare `portfolio` used as an expression (e.g. the
                    // trailing value expression in bond_portfolio.fin) falls
                    // through to ExprItem.  One token of lookahead suffices:
                    // if the token after `portfolio` can be a name AND the
                    // token after that is `{`, treat as a definition.
                    let is_def = Self::token_as_name(self.peek_ahead(1)).is_some()
                        && matches!(self.peek_ahead(2), Token::LBrace);
                    if is_def {
                        match self.parse_portfolio_def() {
                            Some(item) => items.push(item),
                            None => self.skip_to_item_boundary(),
                        }
                    } else {
                        let start_span = self.current_span();
                        match self.parse_expr() {
                            Some(expr) => {
                                let span = start_span.merge(expr.span());
                                items.push(ast::Item::ExprItem(Box::new(expr), span));
                            }
                            None => self.skip_to_item_boundary(),
                        }
                    }
                }
                Token::Let => {
                    match self.parse_let_decl() {
                        Some(item) => items.push(item),
                        None => self.skip_to_item_boundary(),
                    }
                }
                Token::LexError(msg) => {
                    let span = self.current_span();
                    self.errors.push(ParseError::LexErrorBubbled {
                        msg: msg.clone(),
                        span,
                    });
                    self.advance();
                }
                _ => {
                    let start_span = self.current_span();
                    match self.parse_expr() {
                        Some(expr) => {
                            let span = start_span.merge(expr.span());
                            items.push(ast::Item::ExprItem(Box::new(expr), span));
                        }
                        None => self.skip_to_item_boundary(),
                    }
                }
            }
            self.eat_semis();
        }

        let errors = std::mem::take(&mut self.errors);
        ParseResult { items, errors }
    }

    // ── Item parsers ──────────────────────────────────────────────────────────

    /// `fn NAME ( PARAM,* ) -> TYPE BLOCK`
    fn parse_fn_def(&mut self) -> Option<ast::Item> {
        let start = self.current_span();
        self.advance(); // consume `fn`

        let (name, _) = self.expect_ident("function name")?;
        self.expect(&Token::LParen, "'('")?;

        let mut params: Vec<ast::Param> = Vec::new();
        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let param_start = self.current_span();
            let (pname, _) = self.expect_ident("parameter name")?;
            self.expect(&Token::Colon, "':'")?;
            let pty = self.parse_type()?;
            let param_end = self.current_span();
            params.push(ast::Param {
                name: pname,
                ty: pty,
                span: param_start.merge(param_end),
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        self.expect(&Token::Arrow, "'->'")?;
        let return_ty = self.parse_type()?;
        let body = self.parse_block_expr()?;
        let span = start.merge(body.span());
        Some(ast::Item::FnDef {
            name,
            params,
            return_ty,
            body: Box::new(body),
            span,
        })
    }

    /// `portfolio NAME { LEG* }`
    fn parse_portfolio_def(&mut self) -> Option<ast::Item> {
        let start = self.current_span();
        self.advance(); // consume `portfolio`

        let (name, _) = self.expect_ident("portfolio name")?;
        self.expect(&Token::LBrace, "'{'")?;
        self.eat_semis();

        let mut legs: Vec<ast::PortfolioLeg> = Vec::new();
        loop {
            match self.peek() {
                Token::RBrace | Token::Eof => break,
                Token::Long | Token::Short => {
                    match self.parse_portfolio_leg() {
                        Some(leg) => legs.push(leg),
                        None => {
                            // recover: skip to next leg or closing brace
                            loop {
                                match self.peek() {
                                    Token::Long | Token::Short | Token::RBrace | Token::Eof => break,
                                    _ => { self.advance(); }
                                }
                            }
                        }
                    }
                    self.eat_semis();
                }
                _ => {
                    // unexpected token inside portfolio block — skip it
                    let span = self.current_span();
                    let found = self.peek().to_string();
                    self.errors.push(ParseError::UnexpectedToken {
                        expected: "'long', 'short', or '}'".to_owned(),
                        found,
                        span,
                    });
                    self.advance();
                }
            }
        }

        let end = self.current_span();
        self.expect(&Token::RBrace, "'}'")?;
        let span = start.merge(end);
        Some(ast::Item::PortfolioDef { name, legs, span })
    }

    /// `(long | short) EXPR IDENT (at IDENT = EXPR)*`
    fn parse_portfolio_leg(&mut self) -> Option<ast::PortfolioLeg> {
        let start = self.current_span();

        let direction = match self.peek() {
            Token::Long => {
                self.advance();
                ast::LegDirection::Long
            }
            Token::Short => {
                self.advance();
                ast::LegDirection::Short
            }
            _ => {
                let span = self.current_span();
                let found = self.peek().to_string();
                self.errors.push(ParseError::UnexpectedToken {
                    expected: "'long' or 'short'".to_owned(),
                    found,
                    span,
                });
                return None;
            }
        };

        let size = self.parse_expr()?;

        // instrument identifier
        let (instrument, _) = self.expect_ident("instrument name")?;

        // at clauses: `at NAME = EXPR` or `at NAME` (bare identifier — no = EXPR)
        let mut at_clauses: Vec<(String, ast::Expr)> = Vec::new();
        while self.eat(&Token::At) {
            let (clause_name, clause_span) = self.expect_ident("at-clause name")?;
            // Check if followed by `=`
            if self.eat(&Token::Eq) {
                let clause_val = self.parse_expr()?;
                at_clauses.push((clause_name, clause_val));
            } else {
                // bare `at NAME` — treat as `at NAME = Ident(NAME)`
                at_clauses.push((
                    clause_name.clone(),
                    ast::Expr::Ident(clause_name, clause_span),
                ));
            }
        }

        let end = if let Some((_, last_expr)) = at_clauses.last() {
            last_expr.span()
        } else {
            size.span()
        };
        let span = start.merge(end);

        Some(ast::PortfolioLeg {
            direction,
            size,
            instrument,
            at_clauses,
            span,
        })
    }

    /// `let NAME (: TYPE)? = EXPR`  (as a top-level item)
    fn parse_let_decl(&mut self) -> Option<ast::Item> {
        let start = self.current_span();
        self.advance(); // consume `let`

        let (name, _) = self.expect_ident("variable name")?;

        let ty = if self.eat(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::Eq, "'='")?;
        let value = self.parse_expr()?;
        let span = start.merge(value.span());
        Some(ast::Item::LetDecl {
            name,
            ty,
            value: Box::new(value),
            span,
        })
    }

    // ── Block ─────────────────────────────────────────────────────────────────

    /// `{ STMT* EXPR? }` — a brace-enclosed block expression.
    fn parse_block_expr(&mut self) -> Option<ast::Expr> {
        let start = self.current_span();
        self.expect(&Token::LBrace, "'{'")?;
        self.eat_semis();

        let mut stmts: Vec<ast::Stmt> = Vec::new();
        let mut tail: Option<Box<ast::Expr>> = None;

        loop {
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }

            // Try to determine if this is a stmt or a tail expression.
            // We parse speculatively: let/return/for are always statements.
            // Otherwise parse an expression and check for a trailing `;`.
            match self.peek() {
                Token::Let => {
                    match self.parse_let_stmt() {
                        Some(s) => stmts.push(s),
                        None => self.skip_to_stmt_boundary(),
                    }
                    self.eat_semis();
                }
                Token::Return => {
                    match self.parse_return_stmt() {
                        Some(s) => stmts.push(s),
                        None => self.skip_to_stmt_boundary(),
                    }
                    self.eat_semis();
                }
                Token::For => {
                    match self.parse_for_stmt() {
                        Some(s) => stmts.push(s),
                        None => self.skip_to_stmt_boundary(),
                    }
                    self.eat_semis();
                }
                _ => {
                    // parse expr — may be a statement-expression (followed by `;`)
                    // or the block's tail value (not followed by `;`).
                    match self.parse_expr() {
                        Some(expr) => {
                            if self.eat(&Token::Semi) {
                                // it's a statement expression
                                let sp = expr.span();
                                stmts.push(ast::Stmt::Expr(Box::new(expr), sp));
                                self.eat_semis();
                            } else if matches!(self.peek(), Token::RBrace | Token::Eof) {
                                // tail expression — block value
                                tail = Some(Box::new(expr));
                                break;
                            } else {
                                // treat as statement without explicit terminator
                                let sp = expr.span();
                                stmts.push(ast::Stmt::Expr(Box::new(expr), sp));
                                self.eat_semis();
                            }
                        }
                        None => {
                            // recovery
                            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                                break;
                            }
                            self.skip_to_stmt_boundary();
                        }
                    }
                }
            }
        }

        let end = self.current_span();
        self.expect(&Token::RBrace, "'}'")?;
        let span = start.merge(end);
        Some(ast::Expr::Block(stmts, tail, span))
    }

    // ── Statement parsers ─────────────────────────────────────────────────────

    /// `let NAME (: TYPE)? = EXPR`
    fn parse_let_stmt(&mut self) -> Option<ast::Stmt> {
        let start = self.current_span();
        self.advance(); // consume `let`

        let (name, _) = self.expect_ident("variable name")?;
        let ty = if self.eat(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&Token::Eq, "'='")?;
        let value = self.parse_expr()?;
        let span = start.merge(value.span());
        Some(ast::Stmt::Let {
            name,
            ty,
            value: Box::new(value),
            span,
        })
    }

    /// `return EXPR?`
    fn parse_return_stmt(&mut self) -> Option<ast::Stmt> {
        let start = self.current_span();
        self.advance(); // consume `return`

        // If the next token starts an expression, parse it.
        let value = if !matches!(self.peek(), Token::Semi | Token::RBrace | Token::Eof) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = value
            .as_ref()
            .map(|e| e.span())
            .unwrap_or(start);
        let span = start.merge(end);
        Some(ast::Stmt::Return(value, span))
    }

    /// `for VAR in EXPR BLOCK`
    fn parse_for_stmt(&mut self) -> Option<ast::Stmt> {
        let start = self.current_span();
        self.advance(); // consume `for`

        let (var, _) = self.expect_ident("loop variable name")?;
        self.expect(&Token::In, "'in'")?;
        let iter = self.parse_expr()?;
        let body = self.parse_block_expr()?;
        let span = start.merge(body.span());
        Some(ast::Stmt::For {
            var,
            iter: Box::new(iter),
            body: Box::new(body),
            span,
        })
    }

    // ── Expression precedence chain ───────────────────────────────────────────
    //
    // Precedence levels (lowest → highest):
    //   1. `||`
    //   2. `&&`
    //   3. `== != < > <= >=`  (non-chainable)
    //   4. `+ -`
    //   5. `* / %`
    //   6. unary `-` `!`
    //   7. `as TYPE`          (cast, left-associative)
    //   8. postfix: call `f(...)` and index `e[...]`
    //   9. primary

    fn parse_expr(&mut self) -> Option<ast::Expr> {
        self.parse_or()
    }

    /// Level 1: `||`
    fn parse_or(&mut self) -> Option<ast::Expr> {
        let mut lhs = self.parse_and()?;
        while self.peek() == &Token::OrOr {
            self.advance();
            let rhs = self.parse_and()?;
            let span = lhs.span().merge(rhs.span());
            lhs = ast::Expr::BinOp {
                op: ast::BinOpKind::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Some(lhs)
    }

    /// Level 2: `&&`
    fn parse_and(&mut self) -> Option<ast::Expr> {
        let mut lhs = self.parse_comparison()?;
        while self.peek() == &Token::AndAnd {
            self.advance();
            let rhs = self.parse_comparison()?;
            let span = lhs.span().merge(rhs.span());
            lhs = ast::Expr::BinOp {
                op: ast::BinOpKind::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Some(lhs)
    }

    /// Level 3: `== != < > <= >=` — **non-chainable**.
    ///
    /// If a second comparison operator is seen after the first, emit
    /// [`ParseError::ChainedComparison`], consume the tail, and return the
    /// first comparison as a recovery value.
    fn parse_comparison(&mut self) -> Option<ast::Expr> {
        let lhs = self.parse_additive()?;
        let op = match self.peek() {
            Token::EqEq => ast::BinOpKind::Eq,
            Token::NotEq => ast::BinOpKind::NotEq,
            Token::Lt => ast::BinOpKind::Lt,
            Token::Gt => ast::BinOpKind::Gt,
            Token::LtEq => ast::BinOpKind::LtEq,
            Token::GtEq => ast::BinOpKind::GtEq,
            _ => return Some(lhs),
        };
        let op_span = self.current_span();
        self.advance();
        let rhs = self.parse_additive()?;
        let first_span = lhs.span().merge(rhs.span());
        let result = ast::Expr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span: first_span,
        };

        // Non-chainable: detect a second comparison operator.
        let is_cmp = matches!(
            self.peek(),
            Token::EqEq | Token::NotEq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq
        );
        if is_cmp {
            let second_op_span = self.current_span();
            let chain_span = op_span.merge(second_op_span);
            self.errors
                .push(ParseError::ChainedComparison { span: chain_span });
            // consume the rest of the chain so we can continue
            self.advance();
            let _ = self.parse_additive(); // discard
        }

        Some(result)
    }

    /// Level 4: `+ -`
    fn parse_additive(&mut self) -> Option<ast::Expr> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => ast::BinOpKind::Add,
                Token::Minus => ast::BinOpKind::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative()?;
            let span = lhs.span().merge(rhs.span());
            lhs = ast::Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Some(lhs)
    }

    /// Level 5: `* / %`
    fn parse_multiplicative(&mut self) -> Option<ast::Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => ast::BinOpKind::Mul,
                Token::Slash => ast::BinOpKind::Div,
                Token::Percent => ast::BinOpKind::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            let span = lhs.span().merge(rhs.span());
            lhs = ast::Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Some(lhs)
    }

    /// Level 6: unary `-` `!`
    fn parse_unary(&mut self) -> Option<ast::Expr> {
        match self.peek() {
            Token::Minus => {
                let start = self.current_span();
                self.advance();
                let inner = self.parse_cast()?;
                let span = start.merge(inner.span());
                Some(ast::Expr::UnaryOp {
                    op: ast::UnaryOpKind::Neg,
                    expr: Box::new(inner),
                    span,
                })
            }
            Token::Bang => {
                let start = self.current_span();
                self.advance();
                let inner = self.parse_cast()?;
                let span = start.merge(inner.span());
                Some(ast::Expr::UnaryOp {
                    op: ast::UnaryOpKind::Not,
                    expr: Box::new(inner),
                    span,
                })
            }
            _ => self.parse_cast(),
        }
    }

    /// Level 7: `as TYPE` — left-associative cast chain.
    ///
    /// Unary is at level 6 and calls into cast, so `-a as price` parses as
    /// `Neg(Cast(a, Price))` because unary wraps the entire cast result.
    fn parse_cast(&mut self) -> Option<ast::Expr> {
        let mut expr = self.parse_postfix()?;
        while self.eat(&Token::As) {
            let ty = self.parse_type()?;
            let span = expr.span().merge(self.tokens[self.cursor.saturating_sub(1)].span);
            expr = ast::Expr::Cast {
                expr: Box::new(expr),
                ty,
                span,
            };
        }
        Some(expr)
    }

    /// Level 8: postfix — call `f(args)` and index `e[i]`.
    ///
    /// Note: only an `Ident` can be the callee of a call expression.
    /// `f(x)(y)` is not legal in this grammar — the callee must be a bare
    /// name.  This is documented and intentional; the type checker would need
    /// first-class function types to support arbitrary callees.
    fn parse_postfix(&mut self) -> Option<ast::Expr> {
        let mut expr = self.parse_primary()?;

        while self.peek() == &Token::LBracket {
            self.advance(); // `[`
            let index = self.parse_expr()?;
            let close = self.current_span();
            self.expect(&Token::RBracket, "']'")?;
            let span = expr.span().merge(close);
            expr = ast::Expr::Index {
                expr: Box::new(expr),
                index: Box::new(index),
                span,
            };
        }
        Some(expr)
    }

    /// Level 9: primary expressions.
    fn parse_primary(&mut self) -> Option<ast::Expr> {
        match self.peek().clone() {
            // Literals
            Token::IntLit(n) => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Int(n), sp))
            }
            Token::FloatLit(f) => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Float(f), sp))
            }
            Token::True => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Bool(true), sp))
            }
            Token::False => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Bool(false), sp))
            }
            Token::StringLit(s) => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::String(s), sp))
            }
            Token::Call => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Call, sp))
            }
            Token::Put => {
                let sp = self.current_span();
                self.advance();
                Some(ast::Expr::Literal(ast::LiteralKind::Put, sp))
            }

            // Identifier — may be a bare name or a call `f(...)`
            // Also accept contextual keywords (e.g. `portfolio`) as bare
            // identifiers when they appear in expression position.
            ref tok if Self::token_as_name(tok).is_some() => {
                let name = Self::token_as_name(self.peek()).expect("just checked");
                let start = self.current_span();
                self.advance();
                if self.eat(&Token::LParen) {
                    // function call
                    let mut args: Vec<ast::Expr> = Vec::new();
                    while !matches!(self.peek(), Token::RParen | Token::Eof) {
                        let arg = self.parse_expr()?;
                        args.push(arg);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                    let close = self.current_span();
                    self.expect(&Token::RParen, "')'")?;
                    let span = start.merge(close);
                    Some(ast::Expr::Call { name, args, span })
                } else {
                    Some(ast::Expr::Ident(name, start))
                }
            }

            // Parenthesised expression
            Token::LParen => {
                let start = self.current_span();
                self.advance();
                let inner = self.parse_expr()?;
                let close = self.current_span();
                self.expect(&Token::RParen, "')'")?;
                // Preserve the outer span so callers see the parens in the span.
                let span = start.merge(close);
                // Return the inner expression but with the full paren span.
                Some(inner.with_span(span))
            }

            // List literal `[ e, e, ... ]`
            Token::LBracket => {
                let start = self.current_span();
                self.advance();
                let mut elems: Vec<ast::Expr> = Vec::new();
                while !matches!(self.peek(), Token::RBracket | Token::Eof) {
                    let elem = self.parse_expr()?;
                    elems.push(elem);
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                let close = self.current_span();
                self.expect(&Token::RBracket, "']'")?;
                let span = start.merge(close);
                Some(ast::Expr::List(elems, span))
            }

            // Block expression `{ ... }`
            Token::LBrace => self.parse_block_expr(),

            // If expression
            Token::If => self.parse_if_expr(),

            // Lex error bubbled up
            Token::LexError(msg) => {
                let span = self.current_span();
                self.errors.push(ParseError::LexErrorBubbled {
                    msg: msg.clone(),
                    span,
                });
                self.advance();
                None
            }

            // Anything else is unexpected
            _ => {
                let span = self.current_span();
                let found = self.peek().to_string();
                self.errors.push(ParseError::UnexpectedToken {
                    expected: "expression".to_owned(),
                    found,
                    span,
                });
                None
            }
        }
    }

    /// `if COND BLOCK (else BLOCK)?`
    fn parse_if_expr(&mut self) -> Option<ast::Expr> {
        let start = self.current_span();
        self.advance(); // consume `if`
        let cond = self.parse_expr()?;
        let then_branch = self.parse_block_expr()?;
        let else_branch = if self.eat(&Token::Else) {
            Some(Box::new(self.parse_block_expr()?))
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map(|e| e.span())
            .unwrap_or_else(|| then_branch.span());
        let span = start.merge(end);
        Some(ast::Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch,
            span,
        })
    }

    // ── Type annotation parser ────────────────────────────────────────────────

    /// Parse a [`TypeAnnotation`] from the current position.
    fn parse_type(&mut self) -> Option<ast::TypeAnnotation> {
        match self.peek().clone() {
            Token::Price => { self.advance(); Some(ast::TypeAnnotation::Price) }
            Token::Rate => { self.advance(); Some(ast::TypeAnnotation::Rate) }
            Token::Notional => { self.advance(); Some(ast::TypeAnnotation::Notional) }
            Token::Date => { self.advance(); Some(ast::TypeAnnotation::Date) }
            Token::Years => { self.advance(); Some(ast::TypeAnnotation::Years) }
            Token::BasisPoints => { self.advance(); Some(ast::TypeAnnotation::BasisPoints) }
            Token::Bool => { self.advance(); Some(ast::TypeAnnotation::Bool) }
            Token::Int => { self.advance(); Some(ast::TypeAnnotation::Int) }
            // `option_type` is the keyword spelling — accept as a type annotation
            Token::Ident(ref s) if s == "option_type" => {
                self.advance();
                Some(ast::TypeAnnotation::OptionType)
            }
            Token::Ident(name) => {
                self.advance();
                Some(ast::TypeAnnotation::Named(name))
            }
            Token::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(&Token::RBracket, "']'")?;
                Some(ast::TypeAnnotation::List(Box::new(inner)))
            }
            _ => {
                let span = self.current_span();
                let found = self.peek().to_string();
                self.errors.push(ParseError::InvalidTypeAnnotation {
                    span,
                    msg: format!("expected a type keyword, found '{found}'"),
                });
                None
            }
        }
    }
}
