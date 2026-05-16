//! Byte-offset ↔ LSP position conversion.
//!
//! The compiler pipeline records source locations as [`finlang_lexer::Span`]
//! values, which carry **UTF-8 byte offsets** into the original source.  LSP
//! positions, by contrast, are **UTF-16 code units** within a line.  Every
//! cursor coming in from the editor has to be translated one way and every
//! diagnostic going out has to be translated the other.
//!
//! [`Rope`] makes byte ↔ line/byte_in_line arithmetic O(log n); the
//! per-character UTF-16 width walk is bounded by the length of one line,
//! which is always tiny compared to the document.

use finlang_lexer::Span;
use ropey::Rope;
use tower_lsp::lsp_types::{Position, Range};

/// Convert a byte offset into an LSP [`Position`] (UTF-16 columns).
#[must_use]
pub fn byte_offset_to_position(rope: &Rope, byte_off: usize) -> Position {
    let byte_off = byte_off.min(rope.len_bytes());
    let line = rope.byte_to_line(byte_off);
    let line_byte_start = rope.line_to_byte(line);
    let column_bytes = byte_off - line_byte_start;

    // Sum UTF-16 lengths of all chars in the line up to `column_bytes`.
    let char_start = rope.byte_to_char(line_byte_start);
    let char_end = rope.byte_to_char(line_byte_start + column_bytes);
    let slice = rope.slice(char_start..char_end);
    let utf16_col: usize = slice.chars().map(|c| c.len_utf16()).sum();

    Position {
        line: line as u32,
        character: utf16_col as u32,
    }
}

/// Convert an LSP [`Position`] back to a byte offset.
///
/// Clamps to document length on out-of-range input rather than panicking.
#[must_use]
pub fn position_to_byte_offset(rope: &Rope, pos: Position) -> usize {
    let line = (pos.line as usize).min(rope.len_lines().saturating_sub(1));
    let line_byte_start = rope.line_to_byte(line);
    let line_byte_end = if line + 1 < rope.len_lines() {
        rope.line_to_byte(line + 1)
    } else {
        rope.len_bytes()
    };

    let target_utf16 = pos.character as usize;
    let char_start = rope.byte_to_char(line_byte_start);
    let char_end = rope.byte_to_char(line_byte_end);
    let slice = rope.slice(char_start..char_end);

    let mut utf16_seen = 0usize;
    let mut byte_off = line_byte_start;
    for c in slice.chars() {
        if utf16_seen >= target_utf16 {
            break;
        }
        utf16_seen += c.len_utf16();
        byte_off += c.len_utf8();
    }
    byte_off.min(rope.len_bytes())
}

/// Convert a byte-offset [`Span`] into an LSP [`Range`].
#[must_use]
pub fn span_to_range(rope: &Rope, span: Span) -> Range {
    Range {
        start: byte_offset_to_position(rope, span.start),
        end: byte_offset_to_position(rope, span.end),
    }
}
