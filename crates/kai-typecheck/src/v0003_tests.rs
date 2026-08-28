use super::*;
use crate::test_support::parse_ok;

/// Full pipeline up to (and including) typecheck: resolution included,
/// which struct tests need for name tables.
fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
    let src = src.as_ref();
    let ast = parse_ok(src);
    let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
    check_with(&ast, &resolution, std::collections::HashMap::new(), std::collections::HashMap::new())
}

const POINT: &str = "type Point = { x: int32; y: int32; }\n";

fn with_point(body: &str) -> String {
    format!("{POINT}{body}")
}

#[test]
fn accepts_struct_literal_field_read_and_write() {
    let src = with_point(
        "fn main() -> int32 {\n    var p = Point { x: 1, y: 2 };\n    p.x = 10;\n    p.y += p.x;\n    return 0;\n}\n",
    );
    assert!(check_src(src).is_ok());
}

#[test]
fn literal_fields_may_come_in_any_order_but_must_be_complete() {
    let src = with_point("fn main() -> int32 { let p = Point { y: 2, x: 1 }; return 0; }\n");
    assert!(check_src(src).is_ok());
}

#[test]
fn rejects_missing_field_in_literal() {
    let src = with_point("fn main() -> int32 { let p = Point { x: 1 }; return 0; }\n");
    assert!(
        check_src(src)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("missing field `y`"))
    );
}

#[test]
fn rejects_duplicate_and_unknown_literal_fields() {
    let src = with_point(
        "fn main() -> int32 { let p = Point { x: 1, x: 2, z: 3, y: 4 }; return 0; }\n",
    );
    let diags = check_src(src).unwrap_err();
    assert!(diags.iter().any(|d| d.message.contains("more than once")));
    assert!(diags.iter().any(|d| d.message.contains("no field `z`")));
}

#[test]
fn rejects_field_access_on_non_struct() {
    let src = "fn main() -> int32 { let n = 1; let m = n.x; return 0; }";
    assert!(check_src(src).unwrap_err().iter().any(|d| {
        d.message
            .contains("cannot access a field on a value of type `int32`")
    }));
}

#[test]
fn rejects_unknown_field_in_access_chain() {
    let src = with_point(
        "type Line = { start: Point; end: Point; }\nfn main() -> int32 { let l = Line { start: Point { x: 1, y: 2 }, end: Point { x: 3, y: 4 } }; let a = l.start.z; return 0; }\n",
    );
    assert!(
        check_src(src)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("`Point` has no field `z`"))
    );
}

#[test]
fn field_type_mismatch_in_literal_is_strict() {
    // No implicit widening into fields: float64 value into int32 field.
    let src = with_point("fn main() -> int32 { let p = Point { x: 1.5, y: 2 }; return 0; }\n");
    assert!(check_src(src).is_err());
}

#[test]
fn calls_check_count_and_types() {
    let src = "fn add(a: int32, b: int32) -> int32 { return a + b; }
fn main() -> int32 { return add(1, 2); }";
    assert!(check_src(src).is_ok());

    let bad_count = "fn add(a: int32, b: int32) -> int32 { return a + b; }
fn main() -> int32 { return add(1); }";
    assert!(
        check_src(bad_count)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("takes 2 arguments"))
    );

    let bad_type = "fn add(a: int32, b: int32) -> int32 { return a + b; }
fn main() -> int32 { return add(1, true); }";
    assert!(check_src(bad_type).is_err());
}

#[test]
fn calls_resolve_out_of_order_and_recursively() {
    // Signatures are collected before any body lowers, so definition
    // order and direct recursion need no forward declarations.
    let src = "fn main() -> int32 { return twice(21); }
fn twice(n: int32) -> int32 { return n + n; }";
    assert!(check_src(src).is_ok());

    let recursive = "fn main() -> int32 { return fib(9); }
fn fib(n: int32) -> int32 { if n < 2 { return n; } else { return fib(n - 1) + fib(n - 2); } }";
    assert!(check_src(recursive).is_ok());
}

#[test]
fn unknown_function_is_reported() {
    let src = "fn main() -> int32 { return foo(); }";
    assert!(
        check_src(src)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("unknown function `foo`"))
    );
}

#[test]
fn mut_gate_walks_the_root_binding() {
    // immut-rooted write rejected...
    let immut =
        with_point("fn main() -> int32 { let p = Point { x: 1, y: 2 }; p.x = 2; return 0; }\n");
    assert!(
        check_src(immut)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("immutable"))
    );

    // ...and so is writing through an immutable param.
    let param_immut = with_point(
        "fn bump(p: Point) -> unit { p.x += 1; return; }\nfn main() -> int32 { bump(Point { x: 1, y: 2 }); return 0; }\n",
    );
    assert!(
        check_src(param_immut)
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("immutable"))
    );

    // A mut param grants LOCAL copy permission (§9.3).
    let param_mut = with_point(
        "fn bump(mut p: Point) -> unit { p.x += 1; return; }\nfn main() -> int32 { bump(Point { x: 1, y: 2 }); return 0; }\n",
    );
    assert!(check_src(param_mut).is_ok());
}

#[test]
fn struct_params_pass_through_call_sites() {
    let src = with_point(
        "fn sum(p: Point) -> int32 { return p.x + p.y; }\nfn main() -> int32 { return sum(Point { x: 3, y: 4 }); }\n",
    );
    assert!(check_src(src).is_ok());
}
