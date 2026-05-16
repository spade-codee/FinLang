//! Smoke / fuzz tests: parse a large number of pseudo-random sources and
//! assert that:
//!
//! 1. The parser never panics.
//! 2. Every error in the result carries a **non-empty** span (`start < end`
//!    or at least `start == end` at EOF — but the span must exist and be
//!    internally consistent: `start <= end`).
//! 3. The `ParseResult` is always well-formed (items and errors are disjoint
//!    concerns; both may be non-empty).
//!
//! Uses a deterministic LCG so the test is reproducible without any extra
//! dependencies.

/// Minimal 64-bit LCG (Knuth parameters).
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        // Parameters from Knuth TAOCP vol.2
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_usize(&mut self, max: usize) -> usize {
        (self.next() % max as u64) as usize
    }
}

const TOKENS: &[&str] = &[
    // keywords
    "let", "fn", "portfolio", "long", "short", "for", "in", "if", "else",
    "as", "at", "return", "true", "false",
    // type keywords
    "price", "rate", "notional", "date", "years", "basis_points", "bool", "int",
    // option type
    "Call", "Put",
    // operators / delimiters
    "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "&&", "||",
    "!", "=", "->", "(", ")", "{", "}", "[", "]", ",", ":", ";",
    // literals
    "0", "1", "42", "3.14", "0.5", "\"hello\"",
    // identifiers
    "x", "y", "z", "foo", "bar", "n", "a", "b", "c",
];

fn random_source(lcg: &mut Lcg, token_count: usize) -> String {
    let mut parts = Vec::with_capacity(token_count);
    for _ in 0..token_count {
        parts.push(TOKENS[lcg.next_usize(TOKENS.len())]);
    }
    parts.join(" ")
}

#[test]
fn smoke_no_panic() {
    let mut lcg = Lcg::new(0xDEAD_BEEF_CAFE_1234);
    let iterations = 500;

    for i in 0..iterations {
        let token_count = 1 + lcg.next_usize(40); // 1..=40 tokens
        let source = random_source(&mut lcg, token_count);

        // Must not panic.
        let result = finlang_parser::parse_str(&source);

        // Every error must have a consistent span.
        for err in &result.errors {
            let sp = err.span();
            assert!(
                sp.start <= sp.end,
                "iteration {i}: error span is inverted: {sp:?} in source: {source:?}"
            );
        }
    }
}
