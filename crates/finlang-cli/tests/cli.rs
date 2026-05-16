//! Integration tests for the `finlang` binary.
//!
//! Each test spawns the binary located via `CARGO_BIN_EXE_finlang` and
//! inspects its exit status and stdout/stderr.  The tests intentionally
//! avoid driving the REPL because interactive I/O is awkward to script
//! portably; pipeline coverage comes from `run` / `check` / `compile`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the `finlang` binary, resolved by Cargo at compile time.
fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_finlang"))
}

/// Workspace root (`crates/finlang-cli` lives two levels under it).
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .unwrap_or(&manifest)
        .to_path_buf()
}

/// Run the binary with the given args and return `(status_code, stdout, stderr)`.
fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to spawn finlang binary");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_option_pricing_exits_zero() {
    let path = workspace_root().join("examples/option_pricing.fin");
    let (code, _out, err) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr was:\n{err}");
}

#[test]
fn run_option_pricing_prints_expected_value() {
    let path = workspace_root().join("examples/option_pricing.fin");
    let (code, out, err) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr was:\n{err}");
    assert!(
        out.contains("10.20"),
        "expected stdout to contain `10.20`, got:\n{out}"
    );
}

#[test]
fn run_bond_portfolio_prints_expected_value() {
    let path = workspace_root().join("examples/bond_portfolio.fin");
    let (code, out, err) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr was:\n{err}");
    assert!(
        out.contains("10370"),
        "expected stdout to contain `10370`, got:\n{out}"
    );
}

#[test]
fn run_var_calculation_prints_expected_value() {
    let path = workspace_root().join("examples/var_calculation.fin");
    let (code, out, err) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr was:\n{err}");
    assert!(
        out.contains("180.86"),
        "expected stdout to contain `180.86`, got:\n{out}"
    );
}

#[test]
fn check_rejects_dimensional_mismatch() {
    let bad = "let x: price = 1.0 as price\nlet y: rate = 0.05\nlet z = x + y\n";
    let tmp = std::env::temp_dir().join("finlang_bad_check.fin");
    std::fs::write(&tmp, bad).expect("write temp");
    let (code, _out, _err) = run(&["check", "--quiet", tmp.to_str().unwrap()]);
    assert_ne!(code, 0, "type error should produce a non-zero exit");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn compile_writes_object_file() {
    let path = workspace_root().join("examples/option_pricing.fin");
    let out_path = std::env::temp_dir().join("finlang_option_pricing_test.o");
    let _ = std::fs::remove_file(&out_path);

    let (code, _stdout, stderr) = run(&[
        "compile",
        path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr was:\n{stderr}");
    let meta = std::fs::metadata(&out_path).expect("object file should exist");
    assert!(meta.len() > 0, "object file is empty");
    let _ = std::fs::remove_file(&out_path);
}
