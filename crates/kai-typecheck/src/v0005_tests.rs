use super::*;
use crate::test_support::parse_ok;
use kai_tast::{KaiType, TypedExprKind};

fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
    let ast = parse_ok(src.as_ref());
    let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
    check_with(&ast, &resolution, std::collections::HashMap::new())
}

fn first_error(src: impl AsRef<str>) -> String {
    let errs = check_src(src).expect_err("expected type errors");
    errs[0].message.clone()
}

#[test]
fn string_literal_types_as_string() {
    let program = check_src("fn main() -> int32 { let s = \"hi\"; return 0; }").unwrap();
    let init = match &program.fns[0].body.stmts[0] {
        kai_tast::TypedStmt::Let(l) => &l.init,
        other => panic!("expected let, got {other:?}"),
    };
    assert_eq!(init.ty, KaiType::String);
}

#[test]
fn array_literal_unifies_element_type() {
    let program = check_src("fn main() -> int32 { let a = [1, 2, 3]; return 0; }").unwrap();
    let init = match &program.fns[0].body.stmts[0] {
        kai_tast::TypedStmt::Let(l) => &l.init,
        other => panic!("expected let, got {other:?}"),
    };
    assert_eq!(init.ty, KaiType::Array(Box::new(KaiType::Int32)));
}

#[test]
fn mixed_array_elements_rejected() {
    let msg = first_error("fn main() -> int32 { let a = [1, true]; return 0; }");
    assert!(msg.contains("must share one type"), "{msg}");
}

#[test]
fn empty_array_without_annotation_rejected() {
    let msg = first_error("fn main() -> int32 { let a = []; return 0; }");
    assert_eq!(
        msg,
        "empty array literal requires a type annotation"
    );
}

#[test]
fn empty_array_with_annotation_accepted() {
    assert!(check_src(
        "fn main() -> int32 { let a: int64[] = []; return 0; }"
    )
    .is_ok());
}

#[test]
fn index_read_yields_element_type() {
    let program =
        check_src("fn main() -> int32 { let a: int64[] = [7]; let v = a[0]; return 0; }")
            .unwrap();
    match &program.fns[0].body.stmts[1] {
        kai_tast::TypedStmt::Let(l) => {
            assert_eq!(l.init.ty, KaiType::Int64);
            assert!(matches!(
                l.init.kind,
                TypedExprKind::Index { .. }
            ));
        }
        other => panic!("expected let, got {other:?}"),
    }
}

#[test]
fn index_requires_array_base() {
    let msg = first_error("fn main() -> int32 { let x = 5; return x[0]; }");
    assert!(msg.contains("only arrays are indexable"), "{msg}");
}

#[test]
fn index_must_be_integer() {
    let msg =
        first_error("fn main() -> int32 { let a = [1]; return a[true]; }");
    assert!(msg.contains("must be an integer"), "{msg}");
}

#[test]
fn index_write_respects_root_writability() {
    // `let` root rejects writes through ANY projection (§9.3).
    let msg = first_error(
        "fn main() -> int32 { let a = [1]; a[0] = 2; return 0; }",
    );
    assert!(msg.contains("immutable"), "{msg}");

    // `var` root accepts them.
    assert!(
        check_src("fn main() -> int32 { var a = [1]; a[0] = 2; return 0; }").is_ok()
    );
}

#[test]
fn index_write_through_struct_field_follows_root() {
    let src = "type S = { arr: int32[]; }\n\
               fn main() -> int32 { var s = S { arr: [] }; s.arr[0] = 1; return 0; }\n";
    assert!(check_src(src).is_ok());

    let src = "type S = { arr: int32[]; }\n\
               fn main() -> int32 { let s = S { arr: [] }; s.arr[0] = 1; return 0; }\n";
    let msg = first_error(src);
    assert!(msg.contains("immutable"), "{msg}");
}

#[test]
fn for_in_binds_immutable_element_local() {
    let src = "fn take(v: int32) -> unit { return; }\n\
               fn main() -> int32 { for v in [1, 2] { take(v); } return 0; }";
    assert!(check_src(src).is_ok());

    // Writing to the loop variable is rejected: it never owns.
    let src = "fn main() -> int32 { for v in [1, 2] { v = 5; } return 0; }";
    let msg = first_error(src);
    assert!(msg.contains("immutable"), "{msg}");
}

#[test]
fn for_in_requires_array_iterable() {
    let msg = first_error("fn main() -> int32 { for v in 42 { return 0; } }");
    assert!(msg.contains("iterates arrays only"), "{msg}");
}
