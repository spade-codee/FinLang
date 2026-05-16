//! Property-style smoke tests for the lexer.
//!
//! We drive the lexer with 1 000 pseudo-random byte strings produced by a
//! deterministic 64-bit LCG (Knuth multiplicative, same constants used by
//! MMIX).  No external crate is required.
//!
//! The invariants we verify are:
//! 1. `tokenize` never panics.
//! 2. The returned vector is non-empty.
//! 3. The last token is always `Token::Eof`.
//! 4. No token has a span that is out-of-bounds for the input string.
//! 5. Every `Token::Eof` span has `start == end == source.len()`.

use finlang_lexer::{tokenize, Token};

/// A minimal deterministic LCG — no external dep needed.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        // Knuth multiplicative LCG (MMIX parameters)
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn next_usize(&mut self, max_exclusive: usize) -> usize {
        (self.next_u64() as usize) % max_exclusive
    }

    /// Fill `buf` with pseudo-random bytes.
    fn fill(&mut self, buf: &mut [u8]) {
        for b in buf.iter_mut() {
            *b = self.next_u64() as u8;
        }
    }
}

/// Generate a pseudo-random printable/mixed-byte string of length `len`.
fn random_string(lcg: &mut Lcg, len: usize) -> String {
    let mut buf = vec![0u8; len];
    lcg.fill(&mut buf);
    // Keep the string valid UTF-8 by re-encoding via char round-trip.
    // Any byte that would form invalid UTF-8 is replaced with a printable
    // ASCII character from the 0x20-0x7E range.
    buf.iter()
        .map(|&b| {
            if b.is_ascii() {
                b as char
            } else {
                // Map high bytes to printable ASCII (0x20..=0x7e)
                char::from(0x20u8 + (b % 0x5Fu8))
            }
        })
        .collect()
}

#[test]
fn fuzz_smoke_never_panics_always_eof() {
    let mut lcg = Lcg::new(0xDEAD_BEEF_CAFE_1234);
    let iterations = 1_000;

    for i in 0..iterations {
        let len = lcg.next_usize(128); // 0..127 bytes
        let source = random_string(&mut lcg, len);

        // Invariant 1: must not panic.
        let tokens = tokenize(&source);

        // Invariant 2: vector is non-empty (at minimum contains Eof).
        assert!(
            !tokens.is_empty(),
            "iteration {i}: tokenize returned empty vec for source {source:?}"
        );

        // Invariant 3: last token is Eof.
        assert_eq!(
            tokens.last().unwrap().node,
            Token::Eof,
            "iteration {i}: last token is not Eof for source {source:?}"
        );

        // Invariant 4 & 5: spans are within bounds.
        let src_len = source.len();
        for spanned in &tokens {
            let sp = spanned.span;
            assert!(
                sp.start <= src_len,
                "iteration {i}: span.start {} > source length {src_len}",
                sp.start
            );
            assert!(
                sp.end <= src_len,
                "iteration {i}: span.end {} > source length {src_len}",
                sp.end
            );
            assert!(
                sp.start <= sp.end,
                "iteration {i}: span.start {} > span.end {}",
                sp.start,
                sp.end
            );
        }

        // Invariant 5 specifically for Eof.
        let eof_span = tokens.last().unwrap().span;
        assert_eq!(
            eof_span.start, src_len,
            "iteration {i}: Eof span.start {} != source length {src_len}",
            eof_span.start
        );
        assert_eq!(
            eof_span.end, src_len,
            "iteration {i}: Eof span.end {} != source length {src_len}",
            eof_span.end
        );
    }
}

/// Ensure the lexer handles completely empty input correctly.
#[test]
fn fuzz_empty_input() {
    let tokens = tokenize("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].node, Token::Eof);
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 0);
}

/// Ensure the lexer handles a string of only whitespace correctly.
#[test]
fn fuzz_whitespace_only() {
    let tokens = tokenize("   \t\n\r\n   ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].node, Token::Eof);
}

/// Ensure the lexer handles a string of only comment lines correctly.
#[test]
fn fuzz_comments_only() {
    let tokens = tokenize("// line 1\n// line 2\n");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].node, Token::Eof);
}

/// Stress test with longer inputs.
#[test]
fn fuzz_smoke_long_inputs() {
    let mut lcg = Lcg::new(0x1234_5678_9ABC_DEF0);

    for i in 0..100 {
        let len = 512 + lcg.next_usize(512); // 512..1023 bytes
        let source = random_string(&mut lcg, len);

        let tokens = tokenize(&source);

        assert!(
            !tokens.is_empty(),
            "iteration {i}: empty result for long input"
        );
        assert_eq!(
            tokens.last().unwrap().node,
            Token::Eof,
            "iteration {i}: no Eof at end of long input"
        );

        let src_len = source.len();
        for spanned in &tokens {
            let sp = spanned.span;
            assert!(sp.start <= src_len && sp.end <= src_len && sp.start <= sp.end,
                "iteration {i}: invalid span {:?} for source of length {src_len}", sp);
        }
    }
}
