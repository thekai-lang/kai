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

// ---------------------------------------------------------------------------
// v0.0.2: bindings, mutability, control flow, boolean logic, widened types.
// ---------------------------------------------------------------------------

const V002: &str = "v0002/main.kai";

#[test]
fn v002_full_pipeline_matches_golden_ir() {
    let source = fixture(V002);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0002/main.expected.ll");
    if !golden.exists() {
        std::fs::write(&golden, &ir).unwrap();
        panic!("golden file missing — wrote it, re-run the test to compare");
    }

    let expected = std::fs::read_to_string(&golden).unwrap();
    assert_eq!(ir, expected, "generated IR diverged from golden file");
}

#[test]
fn v002_jit_executes_bindings_and_logic() {
    // total = 6*7+1 = 43; `43 >= 42 && !(43 > 43)` holds → returns 42.
    assert_eq!(pipeline::jit(&fixture(V002)).unwrap(), 42);
}

#[test]
fn v002_jit_arithmetic_precedence() {
    let src = "fn main() -> int32 { return 2 + 3 * 4 - 1; }";
    assert_eq!(pipeline::jit(src).unwrap(), 13);
}

#[test]
fn v002_jit_mutation_and_compound_assign() {
    let src = "fn main() -> int32 { var x = 5; x += 3; x = x * 2; return x; }";
    assert_eq!(pipeline::jit(src).unwrap(), 16);
}

#[test]
fn v002_jit_else_branch_taken() {
    let src = "fn main() -> int32 { if 1 < 0 { return 1; } else { return 9; } }";
    assert_eq!(pipeline::jit(src).unwrap(), 9);
}

#[test]
fn v002_jit_short_circuit_skips_division_by_zero() {
    // Eager evaluation would trap on `10 / d`; short-circuiting must skip it.
    let and_src =
        "fn main() -> int32 { let d = 0; if false && 10 / d == 0 { return 1; } return 2; }";
    assert_eq!(pipeline::jit(and_src).unwrap(), 2);

    let or_src = "fn main() -> int32 { let d = 0; if true || 10 / d == 0 { return 3; } return 4; }";
    assert_eq!(pipeline::jit(or_src).unwrap(), 3);
}

#[test]
fn v002_jit_int64_literals_via_annotation() {
    let src = concat!(
        "fn main() -> int32 {",
        " let big: int64 = 5000000000;",
        " let half: int64 = big / 2 + big % 2;",
        " if half == 2500000000 { return 1; }",
        " return 0; }"
    );
    assert_eq!(pipeline::jit(src).unwrap(), 1);
}

#[test]
fn v002_jit_float_arithmetic() {
    let src =
        "fn main() -> int32 { let f = 2.5; if f * 2.0 == 5.0 && f < 3.0 { return 7; } return 0; }";
    assert_eq!(pipeline::jit(src).unwrap(), 7);
}

#[test]
fn v002_jit_unary_minus_and_modulo() {
    let src = "fn main() -> int32 { return -8 + 17 % 5 + 48; }";
    assert_eq!(pipeline::jit(src).unwrap(), 42);
}

#[test]
fn v002_jit_nested_scopes_shadowing() {
    // Inner scope shadows `x` with a mutable binding; outer stays 1.
    let src = "fn main() -> int32 { let x = 1; { var x = 100; x += 1; } return x; }";
    assert_eq!(pipeline::jit(src).unwrap(), 1);
}

#[test]
fn v002_error_assign_to_immutable() {
    let failure =
        pipeline::compile("fn main() -> int32 { let x = 1; x = 2; return x; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("cannot assign to `x`")
    );
}

#[test]
fn v002_error_condition_not_bool() {
    let failure =
        pipeline::compile("fn main() -> int32 { if 1 { return 2; } return 0; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("condition must be `bool`")
    );
}

#[test]
fn v002_error_mixed_arithmetic() {
    // int64 widening via the other operand is allowed by design; int+float
    // never mixes, with or without hints.
    let failure =
        pipeline::compile("fn main() -> int32 { let f = 2.5; return 1 + f; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("requires equal types")
    );
}

#[test]
fn v002_error_undeclared_variable() {
    let failure = pipeline::compile("fn main() -> int32 { return n; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("undeclared variable `n`")
    );
}

#[test]
fn v002_error_invalid_assignment_target_is_parse_phase() {
    let failure = pipeline::compile("fn main() -> int32 { 1 + 1 = 2; return 0; }").unwrap_err();
    assert_eq!(failure.phase, "parse");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("invalid assignment target")
    );
}

#[test]
fn v002_error_duplicate_local_same_scope() {
    let failure =
        pipeline::compile("fn main() -> int32 { let x = 1; let x = 2; return x; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(failure.diagnostics[0].message.contains("already declared"));
}

#[test]
fn v002_error_definite_return_still_enforced_with_if() {
    let failure = pipeline::compile("fn main() -> int32 { if true { return 1; } }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(failure.diagnostics[0].message.contains("has no `return`"));
}

// ---------------------------------------------------------------------------
// v0.0.2.1 hardening: recursion budget, poisoned recovery nodes.
// ---------------------------------------------------------------------------

#[test]
fn v0021_deep_nesting_reports_diagnostic_not_crash() {
    // 50k nested parens used to overflow the native stack before any
    // diagnostic could be produced.
    let mut src = String::from("fn main() -> int32 { return ");
    src.push_str(&"(".repeat(50_000));
    src.push('1');
    src.push_str(&")".repeat(50_000));
    src.push_str("; }");

    let failure = pipeline::compile(&src).unwrap_err();
    assert_eq!(failure.phase, "parse");
    assert!(failure.diagnostics[0].message.contains("nested too deeply"));
}

#[test]
fn v0021_duplicate_declaration_keeps_original_id() {
    // The redeclaration is an error, but references must still resolve to the
    // FIRST binding's slot — no u32::MAX sentinel can leak into codegen.
    let failure =
        pipeline::compile("fn main() -> int32 { let x = 1; let x = 2; return x; }").unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(failure.diagnostics[0].message.contains("already declared"));
}

// ---------------------------------------------------------------------------
// v0.0.3: structs, parameters, calls, field access.
// ---------------------------------------------------------------------------

const V003: &str = "v0003/main.kai";

#[test]
fn v003_full_pipeline_matches_golden_ir() {
    let source = fixture(V003);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0003/main.expected.ll");
    if !golden.exists() {
        std::fs::write(&golden, &ir).unwrap();
        panic!("golden file missing — wrote it, re-run the test to compare");
    }

    let expected = std::fs::read_to_string(&golden).unwrap();
    assert_eq!(ir, expected, "generated IR diverged from golden file");
}

#[test]
fn v003_jit_struct_params_are_by_value() {
    // shift(seg.end, 5) returns 39 AND must not disturb seg.end.x: if the
    // parameter were shared rather than copied, `seg.end.x == 30` would
    // fail after the call and main would return 0.
    assert_eq!(pipeline::jit(&fixture(V003)).unwrap(), 39);
}

#[test]
fn v003_jit_call_results_compose() {
    let src = "\
type Pair = { a: int32; b: int32; }
fn sum(p: Pair) -> int32 { return p.a + p.b; }
fn make(a: int32, b: int32) -> Pair { return Pair { a: a, b: b }; }
fn main() -> int32 { return sum(make(20, 1)) * 2; }";
    assert_eq!(pipeline::jit(src).unwrap(), 42);
}

#[test]
fn v003_parenthesized_struct_literal_in_condition_runs() {
    // NO_STRUCT_LITERAL (§9.3): bare literals in if-conditions read as
    // comparison + block; parentheses lift the ban.
    let src = "\
type Point = { x: int32; y: int32; }
fn main() -> int32 {
    var p = Point { x: 7, y: 8 };
    if (p.x == 7 && p.y == 8) { return 1; }
    return 0;
}";
    assert_eq!(pipeline::jit(src).unwrap(), 1);
}

#[test]
fn v004_string_api_rejects_use_bearing_source() {
    // The in-memory API has no project root, so imports cannot resolve.
    // Predictable user-facing situation -> diagnostic, never an internal
    // error (§8).
    let failure =
        pipeline::compile("use math.extra;\nfn main() -> int32 { return 0; }").unwrap_err();
    assert_eq!(failure.phase, "resolve");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("modules require a file entry point"),
        "got: {}",
        failure.diagnostics[0].message
    );
}

// -- v0.0.4: file entry points and module trees ------------------------------

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

fn temp_project(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kai-e2e-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn file_pipeline_compiles_and_jits_a_module_tree() {
    let root = temp_project("tree");
    let entry = write(
        &root,
        "main.kai",
        "use math.ops;\nfn main() -> int32 { return ops.three(); }",
    );
    write(
        &root,
        "math/ops.kai",
        "public fn three() -> int32 { return 3; }",
    );

    assert_eq!(pipeline::jit_file(&entry).unwrap(), 3);

    let ir = pipeline::compile_file(&entry).unwrap();
    assert!(ir.contains("@math.ops.three"), "ir:\n{ir}");
}

#[test]
fn file_pipeline_type_errors_name_the_imported_file() {
    let root = temp_project("typeerr");
    let entry = write(
        &root,
        "main.kai",
        "use math.ops;\nfn main() -> int32 { return ops.three(); }",
    );
    write(&root, "math/ops.kai", "public fn three() -> bool { return true; }");

    // Entry expects int32 from main; the MODULE body's own mismatch (bool ret
    // is fine for a helper) must surface via three()'s use in main.
    let failure = pipeline::jit_file(&entry).unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure
            .sources
            .iter()
            .any(|(name, src)| name == "math/ops.kai" && src.contains("public fn three"))
    );
    assert!(
        failure.diagnostics[0].message.len() > 0,
        "diagnostics are present"
    );
}

#[test]
fn file_pipeline_reports_missing_module_from_loader() {
    let root = temp_project("ghostmod");
    let entry = write(
        &root,
        "main.kai",
        "use ghost.thing;\nfn main() -> int32 { return 0; }",
    );

    let failure = pipeline::compile_file(&entry).unwrap_err();
    assert_eq!(failure.phase, "resolve");
    assert_eq!(
        failure.diagnostics[0].message,
        "cannot find module `ghost.thing`"
    );
    assert_eq!(failure.diagnostics[0].file.as_deref(), Some("main.kai"));
}

#[test]
fn file_pipeline_rejects_private_access_across_modules() {
    let root = temp_project("private");
    let entry = write(
        &root,
        "main.kai",
        "use util.core;\nfn main() -> int32 { return core.secret(); }",
    );
    write(
        &root,
        "util/core.kai",
        "fn secret() -> int32 { return 42; }",
    );

    let failure = pipeline::jit_file(&entry).unwrap_err();
    assert_eq!(failure.phase, "typecheck");
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.message == "function `core.secret` is not public"),
        "{:?}",
        failure.diagnostics
    );
}

#[test]
fn string_api_still_refuses_use_decls() {
    let failure = pipeline::compile(
        "use util.core;\nfn main() -> int32 { return 0; }",
    )
    .unwrap_err();
    assert_eq!(failure.phase, "resolve");
    assert!(failure.diagnostics[0].message.contains("file entry point"));
}

// -- v0.0.4 fixture: multi-module project ------------------------------------

fn v0004_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0004/main.kai")
}

#[test]
fn v004_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0004_entry()).expect("compilation should succeed");

    let golden = v0004_entry().with_file_name("main.expected.ll");
    if !golden.exists() {
        std::fs::write(&golden, &ir).unwrap();
        panic!("golden file missing — wrote it, re-run the test to compare");
    }

    let expected = std::fs::read_to_string(&golden).unwrap();
    assert_eq!(ir, expected, "generated IR diverged from golden file");
}

#[test]
fn v004_jit_module_tree_returns_expected_value() {
    // 6 + 8 + 7 + 10 across three modules (see main.kai).
    assert_eq!(pipeline::jit_file(&v0004_entry()).unwrap(), 31);
}
