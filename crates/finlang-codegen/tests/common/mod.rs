//! Shared test helpers: lex → parse → typecheck → lower → const_fold → dce pipeline.

use finlang_ir::{const_fold, dce, lower, IrProgram};
use finlang_parser::parse_str;
use finlang_types::check;

/// Run the full frontend pipeline on a `.fin` source snippet and return the
/// optimised [`IrProgram`].
///
/// Panics if any stage fails (lex error, parse error, type error, lower error).
#[allow(dead_code)]
pub fn compile_source(src: &str) -> IrProgram {
    let parsed = parse_str(src);
    let types = check(&parsed.items);
    assert!(
        types.errors.is_empty(),
        "type errors in test source: {:?}",
        types.errors
    );
    let mut prog = lower(&parsed.items, &types)
        .expect("IR lowering failed");
    const_fold(&mut prog);
    dce(&mut prog);
    prog
}

/// Run the full frontend pipeline on a file's contents (read at compile time
/// via [`std::fs::read_to_string`]).
///
/// Panics if the file cannot be read or any compilation stage fails.
#[allow(dead_code)]
pub fn compile_file(path: &str) -> IrProgram {
    let src = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    compile_source(&src)
}
