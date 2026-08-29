use super::*;
use crate::test_support::parse_ok;
use kai_resolver::{ModuleInput, analyze_modules};
use kai_tast::{KaiType, TypedExprKind, TypedStmt};
use kai_ast::Program;

fn check_multi(
    entry_src: &str,
    mod_path: &str,
    mod_src: &str,
) -> Result<TypedProgram, Vec<kai_diagnostics::Diagnostic>> {
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
    let resolution = match analyze_modules(&inputs) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
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
    crate::check_with(&merged, &resolution, std::collections::HashMap::new(), std::collections::HashMap::new())
}

#[test]
fn test_alias_and_qualified_type() {
    let res = check_multi(
        "use entity as e;\nfn main() -> int32 { return 0; } fn test(u: e.User) -> e.User { return e.User.create(); }",
        "entity",
        "public type User = { id: int32; }\npublic fn User.create() -> User { return User { id: 1 }; }"
    );
    assert!(res.is_ok(), "{:?}", res.err());
}

#[test]
fn test_direct_symbol_import() {
    let res = check_multi(
        "use domain.entity.User;\nfn main() -> int32 { return 0; } fn test() -> User { return User.create(); }",
        "domain.entity",
        "public type User = { id: int32; }\npublic fn User.create() -> User { return User { id: 1 }; }"
    );
    assert!(res.is_ok(), "{:?}", res.err());
}

#[test]
fn test_ownership_rule_enforced() {
    let res = check_multi(
        "use entity as e;\nfn main() -> int32 { return 0; } fn User.hack() -> unit {}\n",
        "entity",
        "public type User = { id: int32; }"
    );
    assert!(res.is_err());
    let err = format!("{:?}", res.err());
    assert!(err.contains("must be defined in the same module that owns the type"), "{}", err);
}

#[test]
fn test_private_type_rejected() {
    let res = check_multi(
        "use entity as e;\nfn main() -> int32 { return 0; } fn test() -> e.Secret { return 0; }",
        "entity",
        "type Secret = { id: int32; }"
    );
    assert!(res.is_err());
    let err = format!("{:?}", res.err());
    assert!(err.contains("type `Secret` is private") || err.contains("has no type `Secret`"), "{}", err);
}

#[test]
fn test_value_assignment_rejected() {
    let res = check_multi(
        "use entity;\nfn main() -> int32 { return 0; } fn test() -> unit { let x = entity.User; }",
        "entity",
        "public type User = { id: int32; }"
    );
    assert!(res.is_err());
    let err = format!("{:?}", res.err());
    assert!(err.contains("symbol cannot be used as a value"));
}
