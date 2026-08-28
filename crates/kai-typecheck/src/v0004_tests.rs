use super::*;
use crate::test_support::parse_ok;
use kai_resolver::{ModuleInput, analyze_modules};
use kai_tast::{KaiType, TypedExprKind, TypedStmt};

/// Two-module pipeline: entry ("main.kai") + one loaded module. The
/// dotted module name's last segment becomes its import alias.
fn check_multi(
    entry_src: &str,
    mod_path: &str,
    mod_src: &str,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let entry = parse_ok(entry_src);
    let module = parse_ok(mod_src);
    let inputs = [
        ModuleInput {
            name: "",
            file: "main.kai",
            program: &entry,
        },
        ModuleInput {
            name: mod_path,
            file: &format!("{}.kai", mod_path.replace('.', "/")),
            program: &module,
        },
    ];
    let resolution = analyze_modules(&inputs).expect("resolution failed");
    let merged = Program {
        use_decls: Vec::new(),
        fns: entry.fns.iter().chain(module.fns.iter()).cloned().collect(),
        types: entry
            .types
            .iter()
            .chain(module.types.iter())
            .cloned()
            .collect(),
    };
    check_with(&merged, &resolution, std::collections::HashMap::new())
}

const ENTRY_USE: &str = "use support.util;\n";

#[test]
fn qualified_call_resolves_to_public_fn_of_imported_module() {
    let tast = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return util.three(); }}"),
        "support.util",
        "public fn three() -> int32 { return 3; }",
    )
    .expect("ok");
    // Call targets the GLOBAL id (module decls come after the entry's).
    match &tast.fns[0].body.stmts[0] {
        TypedStmt::Return(Some(expr)) => match &expr.kind {
            TypedExprKind::Call { func, .. } => assert_eq!(func.0, 1),
            other => panic!("expected call, got {other:?}"),
        },
        other => panic!("expected return, got {other:?}"),
    }
}

#[test]
fn private_fn_rejects_qualified_call() {
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return util.helper(); }}"),
        "support.util",
        "fn helper() -> int32 { return 1; }",
    )
    .unwrap_err();
    assert!(
        diags
            .iter()
            .any(|d| d.message == "function `util.helper` is not public")
    );
}

#[test]
fn unknown_member_reports_the_qualified_path() {
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return util.nope(); }}"),
        "support.util",
        "public fn three() -> int32 { return 3; }",
    )
    .unwrap_err();
    assert!(
        diags
            .iter()
            .any(|d| d.message == "unknown function `util.nope`")
    );
}

#[test]
fn unqualified_names_do_not_leak_across_modules() {
    // §3.6: imports never inject into any scope — `three()` alone must
    // stay invisible even though support.util is imported.
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return three(); }}"),
        "support.util",
        "public fn three() -> int32 { return 3; }",
    )
    .unwrap_err();
    assert!(diags.iter().any(|d| d.message == "unknown function `three`"));
}

#[test]
fn same_local_names_in_two_modules_do_not_collide() {
    // Entry sees only the PUBLIC name; the module internally uses its
    // own private `five` — unqualified lookups never cross modules.
    let tast = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return util.five_pub(); }}"),
        "support.util",
        "fn five() -> int32 { return 5; } public fn five_pub() -> int32 { return five(); }",
    )
    .expect("ok");
    assert_eq!(tast.fns.len(), 3);
    // Entry fn keeps the bare symbol; module fns carry their module.
    assert_eq!(tast.fns[1].name, "five");
    assert_eq!(tast.fns[1].module, "support.util");
    assert_eq!(tast.fns[2].name, "five_pub");
    assert_eq!(tast.fns[0].module, "");
}

#[test]
fn qualified_struct_literal_needs_public_type() {
    let src_body =
        "public type Pt = { x: int32; }\npublic fn make() -> Pt { return Pt { x: 1 }; }\n";
    // Public: fine through the alias...
    let tast = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ let p = util.Pt {{ x: 7 }}; return p.x; }}"),
        "support.util",
        src_body,
    )
    .expect("ok");
    match &tast.fns[0].body.stmts[0] {
        TypedStmt::Let(let_) => {
            assert!(matches!(let_.init.ty, KaiType::Struct(_)));
        }
        other => panic!("expected let, got {other:?}"),
    }

    // ...private: rejected with the qualified path.
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ let p = util.Pt {{ x: 7 }}; return p.x; }}"),
        "support.util",
        "type Pt = { x: int32; }\n",
    )
    .unwrap_err();
    assert!(diags.iter().any(|d| d.message == "type `util.Pt` is not public"));
}

#[test]
fn calling_a_field_value_is_not_a_module_call() {
    // `foo.bar()` where foo is NOT an import alias: value semantics.
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return foo.bar(); }}"),
        "support.util",
        "public fn bar() -> int32 { return 1; }",
    )
    .unwrap_err();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("only direct calls"))
    );
}

#[test]
fn diagnostics_inside_module_bodies_carry_that_files_name() {
    let diags = check_multi(
        &format!("{ENTRY_USE}fn main() -> int32 {{ return util.bad(); }}"),
        "support.util",
        "public fn bad() -> int32 { return true; }",
    )
    .unwrap_err();
    let in_module = diags
        .iter()
        .find(|d| d.file.as_deref() == Some("support/util.kai"))
        .expect("error attributed to the module file");
    assert!(in_module.message.contains("`int32`"));
}
