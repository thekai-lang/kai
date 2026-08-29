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

/// Golden-compare with write-on-missing bootstrap, shared by every version's
/// IR test: the first run writes the file and asks for a re-run.
fn assert_golden(rel: &str, ir: &str) {
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel);
    if !golden.exists() {
        std::fs::write(&golden, ir).unwrap();
        panic!("golden file missing — wrote {rel}, re-run the test to compare");
    }
    let expected = std::fs::read_to_string(&golden).unwrap();
    assert_eq!(ir, expected, "generated IR diverged from golden file ({rel})");
}

/// Asserts a source fails in `phase` with a diagnostic containing `needle`.
fn assert_fails_at(source: &str, phase: &str, needle: &str) {
    let failure = pipeline::compile(source).unwrap_err();
    assert_eq!(failure.phase, phase, "source:\n{source}");
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.message.contains(needle)),
        "expected {phase} diagnostic containing {needle:?}, got {:?}",
        failure.diagnostics
    );
}

const MAIN: &str = "v0001/main.kai";

#[test]
fn full_pipeline_matches_golden_ir() {
    let source = fixture(MAIN);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    assert_golden("v0001/main.expected.ll", &ir);
}

#[test]
fn jit_executes_main_and_returns_zero() {
    let source = fixture(MAIN);
    assert_eq!(pipeline::jit(&source).unwrap(), 0);
}

#[test]
fn missing_main_is_reported_at_resolve() {
    assert_fails_at("fn helper() -> int32 { return 1; }", "resolve", "no `main`");
}

#[test]
fn parse_error_stops_before_resolve() {
    assert_fails_at("fn main() -> int32 { return 0 }", "parse", "`");
}

#[test]
fn type_error_reports_literal_range() {
    let source = "fn main() -> int32 { return 2147483648; }";
    assert_fails_at(source, "typecheck", "does not fit");
}

#[test]
fn lex_error_reports_unknown_character() {
    assert_fails_at("fn main() -> int32 { return 0 $ 1; }", "lex", "$");
}

// ---------------------------------------------------------------------------
// v0.0.2: bindings, mutability, control flow, boolean logic, widened types.
// ---------------------------------------------------------------------------

const V002: &str = "v0002/main.kai";

#[test]
fn v002_full_pipeline_matches_golden_ir() {
    let source = fixture(V002);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    assert_golden("v0002/main.expected.ll", &ir);
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
    assert_fails_at(
        "fn main() -> int32 { return n; }",
        "typecheck",
        "undeclared variable `n`",
    );
}

#[test]
fn v002_error_invalid_assignment_target_is_parse_phase() {
    assert_fails_at(
        "fn main() -> int32 { 1 + 1 = 2; return 0; }",
        "parse",
        "invalid assignment target",
    );
}

#[test]
fn v002_error_duplicate_local_same_scope() {
    assert_fails_at(
        "fn main() -> int32 { let x = 1; let x = 2; return x; }",
        "typecheck",
        "already declared",
    );
}

#[test]
fn v002_error_definite_return_still_enforced_with_if() {
    assert_fails_at(
        "fn main() -> int32 { if true { return 1; } }",
        "typecheck",
        "has no `return`",
    );
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

    assert_fails_at(&src, "parse", "nested too deeply");
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

    assert_golden("v0003/main.expected.ll", &ir);
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
        !failure.diagnostics[0].message.is_empty(),
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
        "cannot find module or symbol `ghost.thing`"
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
            .any(|d| d.message == "function `secret` is private"),
        "{:?}",
        failure.diagnostics
    );
}

#[test]
fn string_api_still_refuses_use_decls() {
    assert_fails_at(
        "use util.core;\nfn main() -> int32 { return 0; }",
        "resolve",
        "file entry point",
    );
}

// -- v0.0.4 fixture: multi-module project ------------------------------------

fn v0004_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0004/main.kai")
}

#[test]
fn v004_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0004_entry()).expect("compilation should succeed");

    assert_golden("v0004/main.expected.ll", &ir);
}

#[test]
fn v004_jit_module_tree_returns_expected_value() {
    // 6 + 8 + 7 + 10 across three modules (see main.kai).
    assert_eq!(pipeline::jit_file(&v0004_entry()).unwrap(), 31);
}

// -- v0.0.5 phase D: ownership runtime behavior --------------------------------
// These run real retain/release paths; a double-free or use-after-free
// segfaults the test process, which is the assertion.

#[test]
fn ownership_self_aliased_element_replacement_is_safe() {
    // §9.4: arr[0] = arr[0] must prepare the RHS (retain) before releasing
    // the old element — never read freed memory.
    let src = r#"
fn main() -> int32 {
    var arr = ["aa", "bb"];
    arr[0] = arr[0];
    var total = 0;
    if arr[0] == "aa" { total += 10; }
    if arr[1] == "bb" { total += 5; }
    return total;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 15);
}

#[test]
fn ownership_returned_param_stays_valid() {
    // §9.5 id(): returning a borrowed parameter retains it; both the
    // original and the returned co-owner stay valid and compare equal.
    let src = r#"
fn id(s: string) -> string {
    return s;
}
fn main() -> int32 {
    let name = "kai";
    let out = id(name);
    var total = 0;
    if out == name { total += 7; }
    if out == "kai" { total += 3; }
    return total;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 10);
}

#[test]
fn ownership_for_over_owned_temp_and_borrowed_binding() {
    // §9.9: iteration over a call result transfers into the loop (released
    // at for.end); iteration over a binding borrows. Both stay correct.
    let src = r#"
fn make() -> int32[] {
    return [9, 8, 7];
}
fn sum(a: int32[]) -> int32 {
    var t = 0;
    for v in a { t += v; }
    return t;
}
fn main() -> int32 {
    var total = 0;
    for v in make() { total += v; }
    let kept = make();
    total += sum(kept);
    return total + kept[0];
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 57);
}

#[test]
fn ownership_string_array_elements_survive_copies() {
    // Array-of-strings destructor path: copy via co-ownership, replace one
    // element, verify contents through both aliases.
    let src = r#"
fn main() -> int32 {
    var words = ["one", "two"];
    let alias = words;
    words[1] = "TWO";
    var total = 0;
    if alias[0] == "one" { total += 1; }
    if words[0] == "one" { total += 2; }
    if words[1] == "TWO" { total += 4; }
    if alias[1] == "TWO" { total += 8; }
    return total;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 15);
}

#[test]
fn ownership_struct_string_field_per_field_semantics() {
    // E1: heap-bearing structs copy per field. Binding `named.name` retains
    // the string independently of the struct.
    let src = r#"
type User = { name: string; age: int32; }
fn main() -> int32 {
    var u = User { name: "ada", age: 36 };
    let nick = u.name;
    u.name = "grace";
    var total = 0;
    if nick == "ada" { total += 1; }
    if u.name == "grace" { total += 2; }
    if u.age == 36 { total += 4; }
    return total;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 7);
}

#[test]
fn ownership_loop_scoped_strings_release_each_iteration() {
    // The loop variable borrows; the array owns its elements. Replacing an
    // element mid-iteration releases only the replaced slot.
    let src = r#"
fn main() -> int32 {
    var words = ["x", "y", "z"];
    var count = 0;
    for w in words {
        if w != "" { count += 1; }
        words[count - 1] = w;
    }
    return count * 10 + 3;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 33);
}

#[test]
fn ownership_struct_copy_is_per_field() {
    // E1/§9.5: copying a heap-bearing struct memcpy's the aggregate and
    // retains each heap field — scalars stay independent copies, the
    // string becomes co-owned.
    let src = r#"
type User = { name: string; age: int32; }
fn main() -> int32 {
    var u = User { name: "ada", age: 36 };
    let copy = u;
    u.age = 50;
    var total = 0;
    if copy.name == "ada" { total += 1; }
    if copy.age == 36 { total += 2; }
    if u.age == 50 { total += 4; }
    return total;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 7);
}

// -- v0.0.8.4+ leak fixes: creation-claim balance in nested owned temps ------

#[test]
fn jit_unwrap_or_call_arg_leak_fixed() {
    // Regression: mk(true, "x").unwrap_or("fallback") — the "x" passed to
    // mk was an orphaned creation claim (rc=1 from kai_string_new, never
    // balanced).  hoist_children restructure now materializes it.
    let src = r#"
fn mk(flag: bool, s: string) -> Result<string, string> {
    if flag { return Ok(s); }
    return Err("nope");
}
fn main() -> int32 {
    let picked = mk(true, "x").unwrap_or("fallback");
    let missed = mk(false, "y").unwrap_or("fallback");
    _ = picked;
    _ = missed;
    return 0;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 0);
}

#[test]
fn jit_catch_call_arg_leak_fixed() {
    // Regression: mk(false, "q") catch |e| { e } — same orphaned-claim
    // pattern as unwrap_or, different codegen path (catch.err block).
    let src = r#"
fn mk(flag: bool, s: string) -> Result<string, string> {
    if flag { return Ok(s); }
    return Err("nope");
}
fn main() -> int32 {
    let rescued = mk(false, "q") catch |e| { e };
    _ = rescued;
    return 0;
}
"#;
    assert_eq!(pipeline::jit(src).unwrap(), 0);
}

// -- v0.0.8.6 leak regression fixtures: tests/fixtures/leak/ ------------------

fn leak_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/leak/")
        .join(name)
}

#[test]
fn jit_leak_minimal_and_rhs() {
    // Standalone && rhs leak — "delta" in and.rhs block had no alloca/release.
    // Exit 1: single hit, no loop.
    let src = std::fs::read_to_string(leak_fixture("minimal.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 1);
}

#[test]
fn jit_leak_minimal2_and_rhs_in_loop() {
    // && inside for loop — same rhs leak, 5 iterations.
    // Exit 5: hits increments once per iteration.
    let src = std::fs::read_to_string(leak_fixture("minimal2.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 5);
}

#[test]
fn jit_leak_test3_loop_and_result() {
    // Loop + && + Result mk calls — progressive isolation.
    // Exit 99: 10 iters × 1 check = hits 10, but file uses hits==40 gate → 99.
    let src = std::fs::read_to_string(leak_fixture("test3.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 99);
}

#[test]
fn jit_leak_test4_unwrap_or_pattern() {
    // Loop + unwrap_or + && with Result payloads.
    // Exit 99: 10 iters × 2 checks = hits 20, file uses hits==40 gate → 99.
    let src = std::fs::read_to_string(leak_fixture("test4.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 99);
}

#[test]
fn jit_leak_test5_catch_closure_full() {
    // Full pattern: catch + closure + unwrap_or + && — all leak paths combined.
    // Exit 99: 10 iters × 4 checks = hits 40, matches if hits==40 gate.
    let src = std::fs::read_to_string(leak_fixture("test5.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 99);
}

#[test]
fn jit_leak_stress_heap_full() {
    // Canonical stress test: 40 heap ops per iteration × 10 iterations.
    // ASan-verified 0 leaks. Exit 99: hits==40 → return 99.
    let src = std::fs::read_to_string(leak_fixture("stress_heap.kai")).unwrap();
    assert_eq!(pipeline::jit(&src).unwrap(), 99);
}

// -- v0.0.5 fixture: ownership runtime -----------------------------------------

fn v0005_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0005/main.kai")
}

#[test]
fn v005_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0005_entry()).expect("compilation should succeed");

    assert_golden("v0005/main.expected.ll", &ir);
}

#[test]
fn v005_jit_module_tree_returns_expected_value() {
    // 1 (string equality across paths) + 2 (self-alias replacement)
    // + 4 (mut array param visible to caller) + 47 (sum via for..in)
    // + 10 (per-field struct semantics) — see main.kai.
    assert_eq!(pipeline::jit_file(&v0005_entry()).unwrap(), 64);
}

// ---------------------------------------------------------------------------
// v0.0.5.1: corpus robustness + §10 panic end-to-end through the CLI.
// ---------------------------------------------------------------------------

/// Every fixture in the corpus must flow through the whole pipeline without
/// a Rust panic: success OR structured diagnostics, never a crash.
#[test]
fn corpus_flows_through_pipeline_without_rust_panics() {
    use std::collections::HashSet;

    const PHASES: [&str; 7] = [
        "lex",
        "parse",
        "resolve",
        "typecheck",
        "ownership",
        "effect",
        "codegen",
    ];

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut stack = vec![root.clone()];
    let mut checked = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("fixture dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "kai") {
                let rel = path
                    .strip_prefix(&root)
                    .expect("under fixtures")
                    .display()
                    .to_string();
                match pipeline::compile_file(&path) {
                    Ok(_) => checked.push(format!("{rel}: ok")),
                    Err(failure) => {
                        assert!(
                            PHASES.contains(&failure.phase),                            "{rel}: unknown failure phase {:?}",
                            failure.phase
                        );
                        assert!(
                            !failure.diagnostics.is_empty(),
                            "{rel}: failed with no diagnostics"
                        );
                        checked.push(format!("{rel}: {} diagnostics", failure.diagnostics.len()));
                    }
                }
            }
        }
    }
    // Every version's fixture set must actually be exercised.
    let seen: HashSet<&str> = checked.iter().filter_map(|s| s.split('/').next()).collect();
    for ver in [
        "v0001",
        "v0002",
        "v0003",
        "v0004",
        "v0005",
        "v0006",
        "v0007",
        "v0008",
        "v0009",
        "reversible",
        "leak",
    ] {
        assert!(seen.contains(ver), "{ver} missing from corpus sweep");
    }
}

fn run_cli(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_kai"))
        .args(args)
        .output()
        .expect("spawn kai binary")
}

/// Writes `program` to a temp file, runs it through the CLI, and asserts the
/// §10 contract: exit code 101 plus the mandated stderr shape. In-process JIT
/// would terminate the test runner — panicking programs must be observed
/// from outside.
fn assert_cli_panic(name: &str, program: &str, message: &str) {
    let dir = std::env::temp_dir().join(format!(
        "kai-panic-e2e-{}-{}",
        name,
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.kai");
    std::fs::write(&path, program).expect("write program");

    let out = run_cli(&["run", path.to_str().unwrap()]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(101),
        "expected exit 101, got {:?}\nstderr:\n{stderr}",
        out.status.code()
    );
    assert!(
        stderr.contains(&format!("kai runtime panic: {message}")),
        "missing panic message:\n{stderr}"
    );
    assert!(
        stderr.lines().any(|l| l.starts_with("  at ") && l.contains("main.kai:")),
        "missing location line:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_out_of_bounds_read_panics_with_location() {
    assert_cli_panic(
        "oob",
        r#"fn main() -> int32 {
    var a = [1, 2];
    return a[9];
}
"#,
        "array index out of bounds",
    );
}

#[test]
fn cli_negative_index_is_out_of_bounds() {
    assert_cli_panic(
        "negidx",
        r#"fn main() -> int32 {
    var a = [1, 2];
    var i = 0 - 1;
    return a[i];
}
"#,
        "array index out of bounds",
    );
}

#[test]
fn cli_division_by_zero_panics() {
    assert_cli_panic(
        "div0",
        r#"fn main() -> int32 {
    var z = 0;
    return 1 / z;
}
"#,
        "division by zero",
    );
}

#[test]
fn cli_modulo_by_zero_panics() {
    assert_cli_panic(
        "mod0",
        r#"fn main() -> int32 {
    var z = 0;
    return 7 % z;
}
"#,
        "modulo by zero",
    );
}

#[test]
fn cli_int32_add_overflow_panics() {
    assert_cli_panic(
        "addovf",
        r#"fn main() -> int32 {
    var big = 2147483647;
    return big + 1;
}
"#,
        "integer overflow",
    );
}

#[test]
fn cli_healthy_program_exits_normally() {
    let dir = std::env::temp_dir().join(format!("kai-ok-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.kai");
    std::fs::write(&path, "fn main() -> int32 { return 7; }").expect("write");

    let out = run_cli(&["run", path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(7));
    assert!(out.stderr.is_empty(), "unexpected stderr");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// v0.0.6: Optional/Result/closures/discard (§9.9a/§9.9b).
// ---------------------------------------------------------------------------

fn v0006_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0006/main.kai")
}

#[test]
fn v006_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0006_entry()).expect("compilation should succeed");
    assert_golden("v0006/main.expected.ll", &ir);
}

#[test]
fn v006_jit_module_tree_returns_expected_value() {
    // Some-forwarded `??` (10) + None fallback (5) + unwrap_or on the
    // still-None Optional (2) + captured closure call (22) - 30, with a
    // discarded tagged union.
    assert_eq!(pipeline::jit_file(&v0006_entry()).unwrap(), 2);
}

/// §9.9a evidence at the IR level: `Optional<int32>` never touches the
/// refcount runtime — the decision is compile-time keyed per instantiation.
#[test]
fn v006_scalar_optional_emits_zero_refcount_calls() {
    let src = r#"fn main() -> int32 {
    let a: int32? = Some(1);
    let b: int32? = None;
    return (a ?? 0) + (b ?? 2);
}"#;
    let ir = pipeline::compile(src).expect("compiles");
    assert!(!ir.contains("kai_retain"), "scalar Optional must not retain");
    assert!(!ir.contains("@kai_release"), "scalar Optional must not release");
}

/// §9.9a laziness made observable: the fallback's side effect must NOT run
/// when the left side is Some.
#[test]
fn v006_coalesce_fallback_is_lazy() {
    let src = r#"fn boom() -> int32 { return 100; }
fn main() -> int32 {
    let x: int32? = Some(7);
    return x ?? boom();
}"#;
    assert_eq!(pipeline::jit(src).unwrap(), 7);
}

#[test]
fn v006_discard_of_tagged_union_requires_escape_hatch() {
    assert_fails_at(
        "fn f() -> int32? { return None; }
fn main() -> int32 { f(); return 0; }",
        "typecheck",
        "`_ = expr;`",
    );
}

#[test]
fn v006_bare_none_requires_annotation() {
    assert_fails_at(
        "fn main() -> int32 { let x = None; return 0; }",
        "typecheck",
        "requires a type annotation",
    );
}

#[test]
fn v006_closure_cycle_capture_is_rejected() {
    // §9.10's exact scenario shape: capturing a closure-bearing struct.
    assert_fails_at(
        "type Node = { action: () -> unit; }
fn seed() -> () -> unit { return fn() -> unit { return; }; }
fn main() -> int32 {
    var n = Node { action: seed() };
    n.action = fn() -> unit { let peek = n; return; };
    return 0;
}",
        "typecheck",
        "contains a closure",
    );
}

#[test]
fn v006_coalesce_needs_optional_lhs() {
    assert_fails_at(
        "fn main() -> int32 { let v: int32 = 1 ?? 2; return v; }",
        "typecheck",
        "`??` needs an `Optional`",
    );
}

#[test]
fn v006_let_underscore_never_parses() {
    assert_fails_at(
        "fn main() -> int32 { let _ = 1; return 0; }",
        "parse",
        "a variable name",
    );
}

// ---------------------------------------------------------------------------
// v0.0.7: @local/@wallclock, effects, DurationLit (whitepaper v0.15 §5.1)
// ---------------------------------------------------------------------------

fn v0007_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0007/main.kai")
}

#[test]
fn v007_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0007_entry()).expect("compilation should succeed");
    assert_golden("v0007/main.expected.ll", &ir);
}

#[test]
fn v007_jit_temporal_flow_returns_42() {
    assert_eq!(pipeline::jit_file(&v0007_entry()).unwrap(), 42);
}

#[test]
fn v007_duration_zero_is_type_error() {
    assert_fails_at(
        "fn main() -> int32 { let x: string @local(0ms) = \"hi\"; return 0; }",
        "typecheck",
        "non-zero",
    );
}

#[test]
fn v007_effects_contract_verified() {
    // inferred {escapes} ⊆ declared {} must fail (§5.1.2)
    let _src_unused = 0;
    // This specific `bad` has no direct escapes, so it would not fail — we test a function that declares empty but body calls an escaping function via transitive
    let src2 = "fn esc(t: string) -> unit effects { escapes-local-context } { return; }\nfn bad2(t: string) -> unit effects {} { esc(t); }\nfn main() -> int32 { return 0; }";
    assert_fails_at(src2, "effect", "declared effects");
}

#[test]
fn v007_local_passed_to_escapes_is_effect_error() {
    let src = "fn esc(t: string @local(30m)) -> unit effects { escapes-local-context } { return; }\nfn main() -> int32 { let tok: string @local(30m) = \"hi\"; esc(tok); return 0; }";
    assert_fails_at(src, "effect", "escapes-local-context");
}

#[test]
fn v007_require_observe_non_bool_rejected() {
    assert_fails_at(
        "fn main() -> int32 { require 1; return 0; }",
        "typecheck",
        "condition must be `bool`",
    );
    assert_fails_at(
        "fn main() -> int32 { observe \"x\"; return 0; }",
        "typecheck",
        "condition must be `bool`",
    );
}

// ---------------------------------------------------------------------------
// v0.0.8: require runtime panic + §10.3 debt.log, observe JSONL sink,
// exactly-once evaluation, string-API recording no-op (§5.2, v0.20–v0.22).
// ---------------------------------------------------------------------------

/// CLI-level: failing `require` exits 101 with the §10.3 message shape AND
/// writes the pre-ledger record to `.kai/debt.log` before exiting.
#[test]
fn v008_require_violation_panics_and_writes_debt_log() {
    let dir = std::env::temp_dir().join(format!("kai-req-{}-{}", "viol", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.kai");
    std::fs::write(
        &path,
        "fn main() -> int32 {\n    let age: int32 = 5;\n    require age > 10;\n    return 0;\n}\n",
    )
    .expect("write program");

    let out = run_cli(&["run", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        stderr.contains("kai runtime panic: requirement violated: age > 10"),
        "missing §10.3 message:\n{stderr}"
    );
    assert!(
        stderr.lines().any(|l| l.starts_with("  at ") && l.contains("main.kai:")),
        "missing location line:\n{stderr}"
    );

    // §10.3 sequencing: the debt record exists AFTER exit — flushed write.
    let debt = dir.join(".kai").join("debt.log");
    let contents = std::fs::read_to_string(&debt).expect("debt.log written before panic");
    assert!(contents.contains("\"kind\":\"correctness\""), "{contents}");
    assert!(contents.contains("\"condition\":\"age > 10\""), "{contents}");
    assert!(contents.contains("\"outcome\":false"), "{contents}");
    assert!(contents.starts_with('{') && contents.trim_end().ends_with('}'));
    assert_eq!(contents.lines().count(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v008_require_pass_runs_clean_no_debt_entry() {
    let dir = std::env::temp_dir().join(format!("kai-req-{}-ok", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.kai");
    std::fs::write(&path, "fn main() -> int32 { let a = 5; require a > 1; return 9; }\n")
        .expect("write");

    let out = run_cli(&["run", path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(9));
    let debt = dir.join(".kai").join("debt.log");
    assert!(!debt.exists(), "passing require must not write debt.log");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v008_observe_writes_valid_jsonl_per_evaluation() {
    let dir = std::env::temp_dir().join(format!("kai-obs-{}-{}", "jsonl", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.kai");
    std::fs::write(
        &path,
        "fn main() -> int32 { observe 1 < 2; observe 5 >= 5; return 3; }\n",
    )
    .expect("write");

    let out = run_cli(&["run", path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));

    let log = std::fs::read_to_string(dir.join(".kai").join("observe.log"))
        .expect("observe.log written");
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 2, "two evaluations -> two records: {log}");
    for line in &lines {
        assert!(line.starts_with('{') && line.ends_with('}'), "{line}");
        assert!(!line.contains("\n"), "raw newline must be escaped: {line}");
        assert!(line.contains("\"condition\""));
        assert!(line.contains("\"timestamp\":\"20"), "RFC3339 year prefix");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v008_signal_evaluation_is_exactly_once() {
    // Synchronous, exactly-once (§5.2.1): if the condition were evaluated
    // twice, the second `once(arr)` read would see a[0] == 2 and the
    // require would trap — exit code 0 proves single evaluation.
    let src = concat!(
        "fn once(mut a: int32[]) -> int32 {\n",
        "    let v = a[0];\n",
        "    a[0] = v + 1;\n",
        "    return v;\n",
        "}\n",
        "fn main() -> int32 {\n",
        "    var arr = [1];\n",
        "    require once(arr) == 1;\n",
        "    return 0;\n",
        "}"
    );
    assert_eq!(pipeline::jit(src).unwrap(), 0);
}

#[test]
fn v008_string_api_recording_is_documented_noop() {
    // §5.2.2/v0.21: compile(&str) has no project root — recording is a
    // documented no-op. Proven at IR level: the observe/debt record calls
    // are ABSENT from string-API output (file-API output has them, see
    // v008_require_violation test). The require panic itself still fires —
    // but verifying that trap requires spawning the binary (kai_panic exits
    // the process), which v008_require_violation already covers via CLI.
    let src = "fn main() -> int32 { observe 1 < 2; return 4; }";
    let ir = pipeline::compile(src).expect("compiles");
    assert!(
        !ir.contains("kai_observe_record"),
        "string API must skip observe recording:\\n{ir}"
    );

    let fail_src = "fn main() -> int32 { require false; return 0; }";
    let ir = pipeline::compile(fail_src).expect("compiles");
    assert!(
        !ir.contains("kai_debt_record"),
        "string API must skip debt recording:\\n{ir}"
    );
    assert!(
        ir.contains("requirement violated:"),
        "the panic itself must remain baked in:\\n{ir}"
    );
}

fn v007_wallclock_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0007/wallclock.kai")
}

#[test]
fn v007_wallclock_matches_golden_ir() {
    let ir = pipeline::compile_file(&v007_wallclock_entry()).expect("wallclock should compile");
    assert_golden("v0007/wallclock.expected.ll", &ir);
}

#[test]
fn v007_wallclock_cascade_has_two_releases() {
    // string @wallclock(30m) is unconditionally heap (header + inner string) → two-step cascade
    // This is the "silently correct for int32 @wallclock, silently wrong for string @wallclock" case (§5.1.7)
    let ir = pipeline::compile_file(&v007_wallclock_entry()).expect("wallclock should compile");
    // Construction must allocate the WALLCLOCK header around the inner string —
    // a bare kai_string_new stored into the slot made release type-confuse
    // KaiString as KaiWallclock (memory corruption, not just a leak).
    assert!(
        ir.contains("call ptr @kai_wallclock_new"),
        "construction must wrap the inner in a wallclock header:\n{ir}"
    );
    assert!(
        ir.contains("declare ptr @kai_wallclock_new"),
        "wallclock constructor missing from declarations:\n{ir}"
    );
    // The generated payload dtor cascades into the heap-bearing inner
    // (release inner first), then the runtime frees the header itself.
    assert!(
        ir.contains("kai.dtor.wall_"),
        "payload dtor missing — cascade would silently skip the inner:\n{ir}"
    );
    // main's scope exit releases through the dedicated wallclock releaser.
    assert!(
        ir.contains("call void @kai_wallclock_release"),
        "header release missing:\n{ir}"
    );
}

fn v0008_require_fail_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0008/require_fail.kai")
}

fn v0008_observe_false_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0008/observe_false.kai")
}

/// The negative counterpart of the ok-path fixture — a GENUINELY violating
/// `require` at runtime proves all three §10.3 properties at once: exit 101,
/// the mandated message shape with the raw source-span condition, and a
/// flushed `.kai/debt.log` record whose fields are exactly right.
#[test]
fn v008_require_fail_fixture_traps_and_writes_correct_debt_record() {
    let dir = std::env::temp_dir().join(format!(
        "kai-v008-fail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");

    // Copy the fixture into a fresh temp root so the debt.log we read back
    // is THIS run's, not a leftover from another test.
    let entry = dir.join("require_fail.kai");
    std::fs::copy(v0008_require_fail_entry(), &entry).expect("copy fixture");

    let out = run_cli(&["run", entry.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        stderr.contains("kai runtime panic: requirement violated: age > 10"),
        "missing violation message:\n{stderr}"
    );
    assert!(
        stderr.lines().any(|l| l.contains("require_fail.kai:6:13")),
        "location must point at the require statement:\n{stderr}"
    );

    let debt = std::fs::read_to_string(dir.join(".kai").join("debt.log"))
        .expect("debt.log flushed before panic");
    assert_eq!(debt.lines().count(), 1, "exactly one violation record");
    for field in [
        "\"kind\":\"correctness\"",
        "\"location\":\"require_fail.kai:6:13\"",
        "\"condition\":\"age > 10\"",
        "\"outcome\":false",
    ] {
        assert!(debt.contains(field), "missing {field} in:\n{debt}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// §5.2.2's boolean must be genuinely THREADED into `kai_observe_record` —
/// a false condition records `"outcome":false` and never affects control
/// flow (exit code comes from `return`, not from the observed value).
#[test]
fn v008_observe_false_records_false_outcome() {
    let dir = std::env::temp_dir().join(format!(
        "kai-v008-obsf-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let entry = dir.join("observe_false.kai");
    std::fs::copy(v0008_observe_false_entry(), &entry).expect("copy fixture");

    let out = run_cli(&["run", entry.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(5), "false observe must not trap");

    let log = std::fs::read_to_string(dir.join(".kai").join("observe.log"))
        .expect("observe.log written");
    assert_eq!(log.lines().count(), 1);
    assert!(
        log.contains("\"condition\":\"done == 1\"") && log.contains("\"outcome\":false"),
        "false outcome must be recorded verbatim:\n{log}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Structural IR proof: `observe` compiles to STRAIGHT-LINE code — the
/// record call sits in the same block as the comparison, with no branch
/// between them (§5.2.2: observe never affects control flow). `require`, by
/// contrast, MUST have the conditional branch (only false does something).
#[test]
fn v008_observe_ir_is_unconditional_straight_line() {
    let ir = pipeline::compile_file_with_sink(
        &v0008_observe_false_entry(),
        std::path::Path::new("/kai-fixture-root"),
    )
    .expect("compiles");

    // No observe-specific blocks exist; the record call lives inline.
    assert!(
        !ir.contains("observe.ok") && !ir.contains("observe.viol") && !ir.contains("observe.rec.bb"),
        "observe must not introduce control flow:\n{ir}"
    );
    let main_body = &ir[ir.find("define i32 @main").unwrap()..];
    let icmp_pos = main_body.find("icmp eq").expect("comparison emitted");
    let call_pos = main_body
        .find("call void @kai_observe_record")
        .expect("record call emitted");
    let between = &main_body[icmp_pos..call_pos];
    assert!(
        !between.contains(" br "),
        "branch between comparison and record call — observe became conditional:\n{}",
        between
    );
}

// -- v0.0.8.1 stabilization: BUG-1..4 regressions -----------------------------

/// BUG-2: chained field access on a computed tagged-union rvalue used to
/// emit `undef` silently (field_read's non-place arm). Must return 4.
#[test]
fn v0081_bug2_chained_field_on_unwrap_or_across_fn() {
    let src = concat!(
        "type P = { x: int32; }\n",
        "fn mk(p: P) -> P? { return Some(p); }\n",
        "fn main() -> int32 { let v = mk(P { x: 4 }); return v.unwrap_or(P { x: 99 }).x; }"
    );
    assert_eq!(pipeline::jit(src).unwrap(), 4);
}

/// BUG-1: `return value;` inside for..in must thread the enclosing fn's
/// return type (was hardcoded `unit`).
#[test]
fn v0081_bug1_early_return_inside_for_in() {
    let src = concat!(
        "fn f(a: int32[]) -> int32 {\n",
        "    for x in a {\n",
        "        if x > 0 {\n",
        "            return x;\n",
        "        }\n",
        "    }\n",
        "    return 0;\n",
        "}\n",
        "fn main() -> int32 { return f([1]); }"
    );
    assert_eq!(pipeline::jit(src).unwrap(), 1);
}

/// BUG-3: same undef family as BUG-2 but through assignment-in-loop +
/// Optional var return; also locks that rc equals the returned value.
#[test]
fn v0081_bug3_optional_var_return_through_loop() {
    let src = concat!(
        "type P = { x: int32; }\n",
        "fn mk(ps: P[]) -> P? {\n",
        "    var out: P? = None;\n",
        "    for q in ps {\n",
        "        if q.x > 0 {\n",
        "            out = Some(q);\n",
        "        }\n",
        "    }\n",
        "    return out;\n",
        "}\n",
        "fn main() -> int32 {\n",
        "    let arr = [P { x: 5 }];\n",
        "    let r = mk(arr);\n",
        "    return r.unwrap_or(P { x: 0 }).x;\n",
        "}"
    );
    assert_eq!(pipeline::jit(src).unwrap(), 5);
}

/// BUG-4: catch on a qualified-call result with a plain local tail used to
/// leave %catch.join without a terminator (verifier crash).
#[test]
fn v0081_bug4_catch_join_terminator_present() {
    let src = concat!(
        "fn discounted(total: int64, percent: int64) -> Result<int64, string> {\n",
        "    require percent >= 0;\n",
        "    if percent > 80 {\n",
        "        return Err(\"over limit\");\n",
        "    }\n",
        "    return Ok(total - total * percent / 100);\n",
        "}\n",
        "fn main() -> int32 {\n",
        "    let five_hundred: int64 = 500;\n",
        "    let one_fifty: int64 = 150;\n",
        "    let rescued: int64 = discounted(five_hundred, one_fifty) catch |e| {\n",
        "        five_hundred\n",
        "    };\n",
        "    _ = rescued;\n",
        "    return 0;\n",
        "}"
    );
    // Compilation itself is the assertion — verify() crashed before.
    let ir = pipeline::compile(src).expect("catch.join must be terminated");
    assert!(ir.contains("catch.join"), "{ir}");
}

fn v0008_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0008/main.kai")
}

/// Golden IR with a FIXED fake sink root — the baked `.kai/*.log` path
/// globals stay deterministic across machines (real runs derive the root
/// from the entry's parent, which is machine-dependent).
#[test]
fn v008_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file_with_sink(
        &v0008_entry(),
        std::path::Path::new("/kai-fixture-root"),
    )
    .expect("compilation should succeed");
    assert_golden("v0008/main.expected.ll", &ir);
}

#[test]
fn v008_ir_contains_signal_and_guard_calls() {
    let ir = pipeline::compile_file_with_sink(
        &v0008_entry(),
        std::path::Path::new("/kai-fixture-root"),
    )
    .expect("compilation should succeed");
    assert!(ir.contains("call void @kai_observe_record"), "observe record missing");
    assert!(ir.contains("call void @kai_debt_record"), "debt record missing");
    assert!(
        ir.contains("requirement violated: a > 0"),
        "raw source-span condition text must be baked verbatim (v0.22)"
    );
    assert!(ir.contains(".kai\\observe.log") || ir.contains(".kai/observe.log"),
        "sink path missing from baked globals");
}

fn v007_boundary_fail_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0007/boundary_fail.kai")
}

fn v007_boundary_ok_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0007/boundary_ok.kai")
}

#[test]
fn v007_boundary_fail_is_effect_error() {
    let err = pipeline::compile_file(&v007_boundary_fail_entry()).unwrap_err();
    assert_eq!(err.phase, "effect");
    assert!(err.diagnostics.iter().any(|d| d.message.contains("escapes-local-context")));
}

#[test]
fn v007_boundary_ok_compiles() {
    let ir = pipeline::compile_file(&v007_boundary_ok_entry()).expect("boundary_ok should compile");
    // wallclock @wallclock passed to escapes should NOT be flagged, and should show cascade for wallclock in main
    assert!(ir.contains("call void @good"), "good should be called");
}

/// §5.1.1 boundary rule — the negative path is THE reason @local/@wallclock
/// exists. Crossing shapes must ALL be rejected, each by its own layer:
///
/// 1. `@local` → escaping fn taking `@local` — types line up, so the EFFECT
///    checker's reachability invariant fires with the dedicated boundary
///    diagnostic (covered by `v007_boundary_fail_is_effect_error`).
/// 2. `@local` → escaping fn taking plain `T` — accepted by typecheck's
///    read-widening (§5.1.7: marker drop on a borrow is sound), then the
///    EFFECT checker rejects it via the boundary rule PROPER, keyed on the
///    callee's effects rather than an accidental parameter-type mismatch.
/// 3. Transitive: an intermediate fn that calls an escaper INHERITS
///    {escapes-local-context} via the §5.1.2 fixpoint without declaring it,
///    so passing `@local` into IT is also rejected — at both hops.
///
/// And the counterpart that keeps @local usable: a NON-escaping callee with
/// a plain parameter accepts the tracked value silently (see
/// `v007_local_reads_as_plain_into_non_escaping_call`) — contagion is gated
/// by callee EFFECTS, not by parameter syntax.
#[test]
fn v007_boundary_escaping_plain_callee_rejected_by_boundary_rule() {
    // The literal §5.1.1 scenario shape: escaping fn takes a plain string.
    let src = "fn escapes(t: string) -> unit effects { escapes-local-context } { return; }\n\
               fn bad(t: string @local(30m)) -> unit { escapes(t); return; }\n\
               fn main() -> int32 { let tok: string @local(30m) = \"hi\"; bad(tok); return 0; }";
    let failure = pipeline::compile(src).unwrap_err();
    assert_eq!(
        failure.phase, "effect",
        "enforcement must come from the boundary rule (callee effects), not an accidental param-type mismatch"
    );
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.message.contains("escapes-local-context")
                && d.message.contains("@wallclock")),
        "expected the dedicated boundary diagnostic, got {:?}",
        failure.diagnostics
    );
}

#[test]
fn v007_local_reads_as_plain_into_non_escaping_call() {
    // §5.1's "cheap" promise: a callee that does not escape must accept a
    // tracked value WITHOUT re-annotating its parameters — otherwise @local
    // infects the whole call graph and is unusable in real code.
    let src = "fn log_token_id(t: string) -> unit { return; }\n\
               fn process(t: string @local(30m)) -> unit { log_token_id(t); return; }\n\
               fn main() -> int32 { let tok: string @local(30m) = \"hi\"; process(tok); return 0; }";
    assert_eq!(pipeline::jit(src).unwrap(), 0);
}

// ---------------------------------------------------------------------------
// v0.0.9: reversible functions (§5.3) — commit/unwind regression + maturity.
// ---------------------------------------------------------------------------

fn reversible_fixture(name: &str, use_entry: bool) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/reversible/")
        .join(name);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read reversible fixture {name}: {err}"));
    (path, src)
}

fn v0009_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v0009/main.kai")
}

/// Golden IR for the v0.0.9 reversible commit fixture — locks the
/// kai_reversible_enter/_push/_commit call sites and the per-type snapshot
/// dtors emitted for scalar, string, and array-of-struct mutations.
#[test]
fn v009_file_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file_with_sink(&v0009_entry(), Path::new("/kai-fixture-root"))
        .expect("v0.0.9 golden fixture should compile");
    assert_golden("v0009/main.expected.ll", &ir);
}

/// §5.3 commit of scalar array-element mutations: every snapshot releases its
/// retained OLD claim on commit and the NEW value stays live. Exact JIT value
/// (no OS exit-code truncation).
#[test]
fn v009_commit_scalar_mutations() {
    let (_, src) = reversible_fixture("scalar_commit.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 7);
}

/// §5.3 commit of heap values (string array elements + by-value borrow struct)
/// — regression for the pre-existing ownership UAF: mutating a heap field of a
/// by-value borrow must NOT release the caller's claim. Caller's struct stays
/// untouched; the string-array writes propagate. Exact value 31.
#[test]
fn v009_commit_heap_and_borrow_struct() {
    let (_, src) = reversible_fixture("heap_commit.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 31);
}

/// §5.3 wide-ledger commit: 6 mixed-type mutations (scalar + string +
/// array-of-structs) pushed then committed. Every mutation propagates. Exact
/// value 63.
#[test]
fn v009_commit_wide_heterogeneous_ledger() {
    let (_, src) = reversible_fixture("lifo_commit.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 63);
}

/// §5.3.5 nested reversible activations: each call owns a ledger that commits
/// on its own return; all mutations reach the caller. Exact value 7.
#[test]
fn v009_commit_nested_activations() {
    let (_, src) = reversible_fixture("nested_commit.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 7);
}

/// §5.3 maturity stress: a single activation loops 32 times pushing a long
/// scalar+string+struct ledger, then commits leak-free. Exact value 127.
#[test]
fn v009_stress_single_activation_loop_churn() {
    let (_, src) = reversible_fixture("stress_loop.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 127);
}

/// §5.3.5 maturity stress: 9 deep chained reversible calls, per-activation
/// ledgers with string + array-of-struct churn, all commit. Exact value 1023
/// (the OS truncates exit codes to a byte, so JIT's full value is the check).
#[test]
fn v009_stress_deep_nested_activations() {
    let (_, src) = reversible_fixture("stress_deep.kai", false);
    assert_eq!(pipeline::jit(&src).unwrap(), 1023);
}

/// §5.3 unwind: mutations heterogeneous (string + array-of-structs) then a
/// failing require. §10.1 terminal exit 101, and the unwind must complete with
/// NO refcount underflow (which would abort the process with a different code
/// and message) before panicking.
#[test]
fn v009_unwind_rolls_back_without_underflow() {
    let (path, _) = reversible_fixture("unwind_basic.kai", true);
    let out = run_cli(&["run", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("refcount underflow"),
        "unwind must not double-release:\n{stderr}"
    );
}

/// §5.3 unwind LIFO mid-loop: a long heterogeneous ledger pushed across loop
/// iterations, then a require trips. Unwind must restore LIFO and release each
/// displaced value exactly once (no underflow), exit 101.
#[test]
fn v009_unwind_lifo_mid_loop_no_underflow() {
    let (path, _) = reversible_fixture("unwind_lifo.kai", true);
    let out = run_cli(&["run", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("refcount underflow"),
        "LIFO unwind must not double-release:\n{stderr}"
    );
}

/// §5.3.5 nested unwind: inner reversible panics → inner ledger unwinds, panic
/// propagates to outer whose ledger ALSO unwinds, then terminal exit 101. Both
/// must roll back cleanly with no underflow.
#[test]
fn v009_unwind_nested_activations_no_underflow() {
    let (path, _) = reversible_fixture("nested_unwind.kai", true);
    let out = run_cli(&["run", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("refcount underflow"),
        "nested unwind must not double-release:\n{stderr}"
    );
}

/// §5.3 maturity stress + unwind: 5 deep nested activations, the deepest
/// panics; every ledger unwinds LIFO before the terminal panic. No underflow,
/// exit 101.
#[test]
fn v009_unwind_deep_stress_no_underflow() {
    let (path, _) = reversible_fixture("unwind_deep.kai", true);
    let out = run_cli(&["run", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("refcount underflow"),
        "deep unwind must not double-release:\n{stderr}"
    );
}

/// §5.3 effect rule: a `reversible` fn may only call `reversible` targets or
/// `compensate`-wrapped calls; a plain ordinary call is a compile error.
#[test]
fn v009_effect_rejects_ordinary_call_inside_reversible() {
    assert_fails_at(
        "fn ordinary() -> unit { return; }
fn rev() -> unit reversible { ordinary(); return; }
fn main() -> int32 { return 0; }",
        "effect",
        "must be wrapped in `compensate`",
    );
}

/// §5.3 effect rule: an indirect closure call inside a `reversible` fn is
/// rejected (indirect calls are not `reversible`).
#[test]
fn v009_effect_rejects_indirect_call_inside_reversible() {
    assert_fails_at(
        "fn rev_helper(c: () -> unit) -> unit reversible { c(); return; }
fn main() -> int32 {
    let cb: () -> unit = fn() -> unit { return; };
    rev_helper(cb);
    return 0;
}",
        "effect",
        "indirect closure call",
    );
}

/// §5.3 compensate: the compensation block's calls are exempt from the
/// reversible-wrapper rule (they run on unwind, not the forward path), and the
/// base reversible call executes. Compile + JIT succeed.
#[test]
fn v009_compensate_wrapped_side_effect_runs() {
    let src = "fn refund() -> unit { return; }
fn charge() -> unit { return; }
fn pay(user: int32, fee: int32) -> unit reversible {
    charge() compensate { refund(); };
    return;
}
fn main() -> int32 {
    pay(1, 2);
    return 7;
}";
    assert_eq!(pipeline::jit(src).unwrap(), 7);
}

/// REGRESSION for the pre-existing ownership UAF this suite surfaced: mutating
/// a heap FIELD of a by-value borrowed aggregate must not release the caller's
/// claim. Plain (non-reversible) code — the ownership layer reversible depends
/// on.
#[test]
fn v009_ownership_mutate_by_value_borrow_struct_field_is_safe() {
    let src = "type User = { name: string; age: int32; }
fn edit(mut u: User) -> unit {
    u.name = \"grace\";
    return;
}
fn main() -> int32 {
    var u = User { name: \"ada\", age: 30 };
    edit(u);
    var t = 0;
    if u.name == \"ada\" { t += 1; }
    if u.age == 30 { t += 2; }
    return t;
}";
    assert_eq!(pipeline::jit(src).unwrap(), 3);
}

/// REGRESSION for the same UAF on a bare by-value borrowed string param.
#[test]
fn v009_ownership_reassign_by_value_borrow_string_param_is_safe() {
    let src = "fn check(mut s: string) -> int32 {
    s = \"zzz\";
    if s == \"zzz\" { return 1; }
    return 0;
}
fn main() -> int32 {
    var name = \"ada\";
    check(name);
    if name == \"ada\" { return 2; }
    return 0;
}";
    assert_eq!(pipeline::jit(src).unwrap(), 2);
}

/// REGRESSION: array-element mutation through a BORROWED array param still
/// releases the OLD element (the array owns element claims even under borrow)
/// — the fix must not suppress release-old for array-element writes.
#[test]
fn v009_ownership_borrowed_array_element_mutation_still_releases() {
    let src = "fn bump(mut a: int32[]) -> unit {
    a[0] = 99;
    return;
}
fn main() -> int32 {
    var a = [1, 2];
    bump(a);
    return a[0];
}";
    assert_eq!(pipeline::jit(src).unwrap(), 99);
}

// ---------------------------------------------------------------------------
// v0.0.7: @local/@wallclock, effects, DurationLit etc. (continued below)
// ---------------------------------------------------------------------------

#[test]
fn v007_boundary_transitive_inference_rejects_whole_chain() {
    // `middle` never declares effects, but it CALLS an escaper — §5.1.2's
    // transitive inference must mark it {escapes-local-context}, so BOTH the
    // inner hop AND main→middle are rejected. One silent hop would make the
    // whole guarantee decorative.
    let src = "fn inner_escape(t: string @local(30m)) -> unit effects { escapes-local-context } { return; }\n\
               fn middle(t: string @local(30m)) -> unit { inner_escape(t); return; }\n\
               fn main() -> int32 { let tok: string @local(30m) = \"hi\"; middle(tok); return 0; }";
    let failure = pipeline::compile(src).unwrap_err();
    assert_eq!(failure.phase, "effect");
    let hits = failure
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("escapes-local-context"))
        .count();
    assert!(
        hits >= 2,
        "both hops (inner_escape + inferred middle) must be flagged, got {hits}:\n{:?}",
        failure.diagnostics
    );
}

#[test]
fn v009_compensate_rejects_mutation_of_outer_variable() {
    assert_fails_at(
        r#"
        fn effect() -> int32 { return 1; }
        fn main() -> int32 reversible {
            var x = "A";
            effect() compensate {
                x = "B";
            };
            return 1;
        }
        "#,
        "typecheck",
        "cannot assign to outer variable 'x' inside compensate block",
    );
}
