//! Error types for the FinLang type checker.
//!
//! All variants carry enough information for [`render_error`] to produce a
//! rich codespan-reporting diagnostic with precise source locations and
//! helpful notes.

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::NoColor;
use finlang_lexer::Span;
use finlang_parser::ast::BinOpKind;
use thiserror::Error;

use crate::ty::FinType;

/// Every error the type checker can produce.
///
/// Variants are ordered from most to least common for match-arm readability.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TypeError {
    /// A binary operation is dimensionally incoherent.
    ///
    /// The `custom_msg` field holds a tailored explanation when the rule
    /// table provides one (e.g. "adding two dates is dimensionally invalid").
    /// When absent the checker generates a generic message from the operand
    /// types.
    #[error("dimensional type mismatch: cannot apply `{op:?}` to `{lhs}` and `{rhs}`")]
    Dimensional {
        /// Left-hand operand type.
        lhs: FinType,
        /// The operator.
        op: BinOpKind,
        /// Right-hand operand type.
        rhs: FinType,
        /// Span of the left-hand operand.
        lhs_span: Span,
        /// Span of the right-hand operand.
        rhs_span: Span,
        /// Span of the full binary expression.
        span: Span,
        /// Optional tailored diagnostic from the rules table.
        custom_msg: Option<&'static str>,
    },

    /// A function was called with an argument of the wrong type.
    #[error("argument {arg_index} of `{fn_name}` has wrong type: expected `{expected}`, found `{found}`")]
    MismatchedArgument {
        /// The function being called.
        fn_name: String,
        /// Zero-based argument index.
        arg_index: usize,
        /// Expected type according to the stdlib signature.
        expected: FinType,
        /// Actual type of the supplied argument.
        found: FinType,
        /// Source span of the argument expression.
        span: Span,
    },

    /// A function was called with the wrong number of arguments.
    #[error("`{fn_name}` expects {expected} arguments, but {found} were supplied")]
    WrongArity {
        /// The function being called.
        fn_name: String,
        /// Number of parameters the function declares.
        expected: usize,
        /// Number of arguments actually supplied.
        found: usize,
        /// Source span of the call expression.
        span: Span,
    },

    /// A call to a function that does not exist in the stdlib or local scope.
    #[error("unknown function `{name}`")]
    UnknownFunction {
        /// The name that was called.
        name: String,
        /// Source span of the call expression.
        span: Span,
    },

    /// A reference to an identifier not in scope.
    #[error("unknown identifier `{name}`")]
    UnknownIdentifier {
        /// The identifier name.
        name: String,
        /// Source span of the identifier.
        span: Span,
    },

    /// The then-branch and else-branch of an `if` expression disagree on type.
    #[error("if branches have incompatible types: then is `{then_ty}`, else is `{else_ty}`")]
    IfBranchMismatch {
        /// Type of the then-branch.
        then_ty: FinType,
        /// Type of the else-branch.
        else_ty: FinType,
        /// Source span of the full `if` expression.
        span: Span,
    },

    /// The condition of an `if` expression is not boolean.
    #[error("if condition must be `bool`, found `{found}`")]
    IfConditionNotBool {
        /// The actual type of the condition.
        found: FinType,
        /// Source span of the condition expression.
        span: Span,
    },

    /// A cast (`expr as TYPE`) that the type checker forbids.
    #[error("invalid cast from `{from}` to `{to}`")]
    InvalidCast {
        /// The source type.
        from: FinType,
        /// The target type.
        to: FinType,
        /// Source span of the cast expression.
        span: Span,
    },

    /// A numeric literal whose dimension could not be resolved.
    ///
    /// Emitted when a bare `5.0` or `42` survives to the top level without
    /// a surrounding annotation or cast.
    #[error("numeric literal needs a financial dimension — add a type annotation (`: price`) or a cast (`as price`)")]
    UnresolvedLiteralType {
        /// Source span of the literal.
        span: Span,
    },

    /// Elements of a list literal have inconsistent types.
    #[error("list element {index} has type `{found}`, expected `{expected}`")]
    ListElementMismatch {
        /// Type of the first element (the expected type for the list).
        expected: FinType,
        /// Actual type of this element.
        found: FinType,
        /// Zero-based element index.
        index: usize,
        /// Source span of the mismatched element.
        span: Span,
    },

    /// The index in `expr[index]` is not an `int`.
    #[error("list index must be `int`, found `{found}`")]
    IndexNotInt {
        /// Actual type of the index expression.
        found: FinType,
        /// Source span of the index expression.
        span: Span,
    },

    /// An index expression `expr[i]` was applied to a non-list type.
    #[error("cannot index into a value of type `{found}` (expected a list)")]
    IndexedNonList {
        /// Actual type of the collection expression.
        found: FinType,
        /// Source span of the collection expression.
        span: Span,
    },
}

impl TypeError {
    /// Return the primary source span of this error.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            TypeError::Dimensional { span, .. }
            | TypeError::WrongArity { span, .. }
            | TypeError::UnknownFunction { span, .. }
            | TypeError::UnknownIdentifier { span, .. }
            | TypeError::IfBranchMismatch { span, .. }
            | TypeError::IfConditionNotBool { span, .. }
            | TypeError::InvalidCast { span, .. }
            | TypeError::UnresolvedLiteralType { span }
            | TypeError::ListElementMismatch { span, .. }
            | TypeError::IndexNotInt { span, .. }
            | TypeError::IndexedNonList { span, .. }
            | TypeError::MismatchedArgument { span, .. } => *span,
        }
    }

    /// Build a [`codespan_reporting`] `Diagnostic<()>` for this error.
    fn to_diagnostic(&self) -> Diagnostic<()> {
        match self {
            TypeError::Dimensional {
                lhs,
                op,
                rhs,
                lhs_span,
                rhs_span,
                span: _,
                custom_msg,
            } => {
                let op_sym = op_symbol(*op);
                let msg = custom_msg.map_or_else(
                    || {
                        format!(
                            "dimensional type mismatch: cannot apply `{op_sym}` \
                             to `{lhs}` and `{rhs}`"
                        )
                    },
                    |m| format!("dimensional type mismatch: {m}"),
                );

                let mut labels = vec![
                    Label::primary((), lhs_span.start..lhs_span.end)
                        .with_message(lhs.to_string()),
                    Label::secondary((), rhs_span.start..rhs_span.end)
                        .with_message(rhs.to_string()),
                ];

                // Suggest multiplication for the most common mistake.
                let mut notes = Vec::new();
                if *op == BinOpKind::Add
                    && *lhs == FinType::Price
                    && *rhs == FinType::Rate
                {
                    notes.push(
                        "hint: did you mean to multiply? \
                         `spot * vol` gives `price`"
                            .to_owned(),
                    );
                    // Swap label order so the primary points at lhs, secondary
                    // at rhs with the type name (codespan renders primary first).
                    labels.clear();
                    labels.push(
                        Label::primary((), lhs_span.start..lhs_span.end)
                            .with_message(lhs.to_string()),
                    );
                    labels.push(
                        Label::secondary((), rhs_span.start..rhs_span.end)
                            .with_message(rhs.to_string()),
                    );
                }

                let mut diag = Diagnostic::error()
                    .with_code("E001")
                    .with_message(msg)
                    .with_labels(labels);
                for n in notes {
                    diag = diag.with_notes(vec![n]);
                }
                diag
            }

            TypeError::MismatchedArgument {
                fn_name,
                arg_index,
                expected,
                found,
                span,
            } => Diagnostic::error()
                .with_code("E002")
                .with_message(format!(
                    "argument {} of `{fn_name}` has wrong type",
                    arg_index + 1
                ))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message(format!("expected `{expected}`, found `{found}`"))])
                .with_notes(vec![format!(
                    "argument {} of `{fn_name}` must be `{expected}`",
                    arg_index + 1
                )]),

            TypeError::WrongArity {
                fn_name,
                expected,
                found,
                span,
            } => Diagnostic::error()
                .with_code("E003")
                .with_message(format!(
                    "`{fn_name}` expects {expected} argument{}, but {found} {} supplied",
                    if *expected == 1 { "" } else { "s" },
                    if *found == 1 { "was" } else { "were" },
                ))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message("wrong number of arguments")]),

            TypeError::UnknownFunction { name, span } => Diagnostic::error()
                .with_code("E004")
                .with_message(format!("unknown function `{name}`"))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message("not found in stdlib or local scope")]),

            TypeError::UnknownIdentifier { name, span } => Diagnostic::error()
                .with_code("E005")
                .with_message(format!("unknown identifier `{name}`"))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message("not declared in any enclosing scope")]),

            TypeError::IfBranchMismatch {
                then_ty,
                else_ty,
                span,
            } => Diagnostic::error()
                .with_code("E006")
                .with_message("if branches have incompatible types")
                .with_labels(vec![Label::primary((), span.start..span.end).with_message(
                    format!("then: `{then_ty}`, else: `{else_ty}`"),
                )]),

            TypeError::IfConditionNotBool { found, span } => Diagnostic::error()
                .with_code("E007")
                .with_message("if condition must be `bool`")
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message(format!("found `{found}`"))]),

            TypeError::InvalidCast { from, to, span } => Diagnostic::error()
                .with_code("E008")
                .with_message(format!("invalid cast from `{from}` to `{to}`"))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message("cast not permitted")]),

            TypeError::UnresolvedLiteralType { span } => Diagnostic::error()
                .with_code("E009")
                .with_message("unresolved numeric literal type")
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message("type cannot be inferred here")])
                .with_notes(vec![
                    "add a type annotation (`: price`) or a cast (`as price`)".to_owned(),
                ]),

            TypeError::ListElementMismatch {
                expected,
                found,
                index,
                span,
            } => Diagnostic::error()
                .with_code("E010")
                .with_message(format!(
                    "list element {index} has type `{found}`, expected `{expected}`"
                ))
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message(format!("found `{found}`"))]),

            TypeError::IndexNotInt { found, span } => Diagnostic::error()
                .with_code("E011")
                .with_message("list index must be `int`")
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message(format!("found `{found}`"))]),

            TypeError::IndexedNonList { found, span } => Diagnostic::error()
                .with_code("E012")
                .with_message("cannot index into a non-list")
                .with_labels(vec![Label::primary((), span.start..span.end)
                    .with_message(format!("type is `{found}`"))]),
        }
    }
}

/// Return the source symbol for a `BinOpKind`.
fn op_symbol(op: BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "/",
        BinOpKind::Mod => "%",
        BinOpKind::Eq => "==",
        BinOpKind::NotEq => "!=",
        BinOpKind::Lt => "<",
        BinOpKind::Gt => ">",
        BinOpKind::LtEq => "<=",
        BinOpKind::GtEq => ">=",
        BinOpKind::And => "&&",
        BinOpKind::Or => "||",
    }
}

/// Render `error` against `source` without ANSI colour codes and return the
/// result as a plain `String`.
///
/// The diagnostic format matches the codespan-reporting default.  Column
/// numbers are 1-based; the source line is printed with carets under the
/// labelled regions.
///
/// # Arguments
///
/// * `file_name` — shown in the diagnostic header (e.g. `"source.fin"`).
/// * `source`    — the original source text.
/// * `error`     — the error to render.
///
/// # Examples
///
/// ```rust
/// use finlang_types::{check_str, render_error};
///
/// let result = check_str("let x: price = 5.0 as price\n\
///                         let y: rate = 0.05\n\
///                         x + y");
/// if let Some(err) = result.errors.first() {
///     let s = render_error("test.fin", "x + y", err);
///     assert!(s.contains("E001"));
/// }
/// ```
#[must_use]
pub fn render_error(file_name: &str, source: &str, error: &TypeError) -> String {
    let file = SimpleFile::new(file_name, source);
    let diag = error.to_diagnostic();
    let config = term::Config::default();
    let mut buf = Vec::<u8>::new();
    {
        let mut writer = NoColor::new(&mut buf);
        term::emit(&mut writer, &config, &file, &diag).unwrap_or_default();
    }
    String::from_utf8(buf).unwrap_or_default()
}
