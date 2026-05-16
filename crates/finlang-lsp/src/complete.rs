//! Completion handler — assembles items from four sources:
//!
//! 1. Reserved keywords.
//! 2. Built-in type names.
//! 3. Stdlib function names (with their signatures shown in `detail`).
//! 4. User-defined identifiers (top-level `let` / `fn` names from the
//!    latest successful parse — no scope-precise tracking in v0.1).

use finlang_parser::ast::Item;
use finlang_types::stdlib_sigs::{lookup_stdlib, stdlib_names};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind};

use crate::analyze::Analysis;

/// All reserved keywords offered by completion.
const KEYWORDS: &[&str] = &[
    "let", "fn", "if", "else", "match", "for", "in", "return", "as",
    "true", "false", "portfolio",
];

/// All built-in financial / scalar type names.
const TYPE_NAMES: &[&str] = &[
    "price", "rate", "notional", "date", "years", "basis_points",
    "bool", "int",
];

/// Literal-constant identifiers (`Call`, `Put`).
const CONSTANTS: &[&str] = &["Call", "Put"];

/// Build a complete completion list for the document.
#[must_use]
pub fn completions(analysis: &Analysis) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    for kw in KEYWORDS {
        items.push(CompletionItem {
            label: (*kw).to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }

    for ty in TYPE_NAMES {
        items.push(CompletionItem {
            label: (*ty).to_owned(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some("financial type".to_owned()),
            ..Default::default()
        });
    }

    for k in CONSTANTS {
        items.push(CompletionItem {
            label: (*k).to_owned(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("OptionType".to_owned()),
            ..Default::default()
        });
    }

    for name in stdlib_names() {
        let sig = lookup_stdlib(name);
        let detail = sig.map(|s| {
            let params: Vec<String> = s.params.iter().map(|p| format!("{p}")).collect();
            format!("fn {name}({}) -> {}", params.join(", "), s.ret)
        });
        items.push(CompletionItem {
            label: name.to_owned(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail,
            documentation: stdlib_doc(name).map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.to_owned(),
                })
            }),
            ..Default::default()
        });
    }

    // User-defined identifiers from top-level items.
    for item in &analysis.items {
        if let Some((name, kind)) = item_completion(item) {
            items.push(CompletionItem {
                label: name,
                kind: Some(kind),
                ..Default::default()
            });
        }
    }

    items
}

/// Extract `(name, kind)` from a top-level `Item` if it introduces a binding.
fn item_completion(item: &Item) -> Option<(String, CompletionItemKind)> {
    match item {
        Item::LetDecl { name, .. } => Some((name.clone(), CompletionItemKind::VARIABLE)),
        Item::FnDef { name, .. } => Some((name.clone(), CompletionItemKind::FUNCTION)),
        Item::PortfolioDef { name, .. } => Some((name.clone(), CompletionItemKind::STRUCT)),
        Item::ExprItem(_, _) => None,
    }
}

/// Short doc string for built-in stdlib functions.
fn stdlib_doc(name: &str) -> Option<&'static str> {
    Some(match name {
        "black_scholes" => "Black-Scholes European option price.",
        "bs_delta" => "First-order spot sensitivity (ΔV/ΔS).",
        "bs_gamma" => "Second-order spot sensitivity (Δ²V/ΔS²).",
        "bs_vega" => "Sensitivity to volatility (ΔV/Δσ).",
        "bs_theta" => "Sensitivity to time decay (-ΔV/Δt).",
        "bs_rho" => "Sensitivity to the risk-free rate (ΔV/Δr).",
        "implied_vol" => "Newton-Raphson solver for implied volatility.",
        "bond_price" => "Present value of a fixed-rate bond's cash flows.",
        "bond_duration" => "Macaulay duration of a fixed-rate bond.",
        "pv01" => "Dollar value of a 1-basis-point parallel yield shift.",
        "discount_factor" => "Continuous-compounding discount factor `exp(-r*t)`.",
        _ => return None,
    })
}
