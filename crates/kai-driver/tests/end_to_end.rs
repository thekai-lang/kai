//! End-to-end tests for v0.0.1: full pipeline over golden fixtures, plus JIT
//! execution and per-phase failure diagnostics.

use kai_driver::pipeline;
use std::path::PathBuf;

fn fixture(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read fixture {}: {err}", path.display()))
}

const MAIN: &str = "v0001/main.kai";

#[test]
fn full_pipeline_matches_golden_ir() {
    let source = fixture(MAIN);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0001/main.expected.ll");
    if !golden.exists() {
        std::fs::write(&golden, &ir).unwrap();
        panic!("golden file missing — wrote it, re-run the test to compare");
    }

    let expected = std::fs::read_to_string(&golden).unwrap();
    assert_eq!(ir, expected, "generated IR diverged from golden file");
}

#[test]
fn jit_executes_main_and_returns_zero() {
    let source = fixture(MAIN);
    assert_eq!(pipeline::jit(&source).unwrap(), 0);
}

#[test]
fn missing_main_is_reported_at_resolve() {
    let failure = pipeline::compile("fn helper() -> int32 { return 1; }").unwrap_err();
    assert_eq!(failure.phase, "resolve");
    assert!(failure.diagnostics[0].message.contains("no `main`"));
}

#[test]
fn parse_error_stops_before_resolve() {
    let failure = pipeline::compile("fn main() -> int32 { return 0 }").unwrap_err();
    assert_eq!(failure.phase, "parse");
    assert!(failure.diagnostics[0].message.contains('`'));
}

#[test]
fn type_error_reports_literal_range() {
    let source = "fn main() -> int32 { return 2147483648; }";
    let failure = pipeline::compile(source).unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(failure.diagnostics[0].message.contains("does not fit"));
}

#[test]
fn lex_error_reports_unknown_character() {
    let failure = pipeline::compile("fn main() -> int32 { return 0 @ 1; }").unwrap_err();
    assert_eq!(failure.phase, "lex");
    assert!(failure.diagnostics[0].message.contains("@"));
}
