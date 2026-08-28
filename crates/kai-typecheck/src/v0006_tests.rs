use super::*;
use crate::test_support::parse_ok;

fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
    let ast = parse_ok(src.as_ref());
    let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
    check_with(&ast, &resolution, std::collections::HashMap::new(), std::collections::HashMap::new())
}

fn first_error(src: impl AsRef<str>) -> String {
    let diags = check_src(src).unwrap_err();
    diags[0].message.clone()
}

#[test]
fn some_and_typed_none_are_accepted() {
    let src = "fn main() -> int32 {
let x: string? = None;
let y: string? = Some(\"kai\");
let z: int32? = Some(1 + 2);
return 0;
}";
    assert!(check_src(src).is_ok());
}

#[test]
fn bare_none_requires_annotation() {
    let msg = first_error("fn main() -> int32 { let x = None; return 0; }");
    assert!(msg.contains("requires a type annotation"), "{msg}");
}

#[test]
fn coalesce_yields_payload_type() {
    let src = "fn main() -> int32 {
let x: string? = None;
let v: string = x ?? \"fallback\";
return 0;
}";
    assert!(check_src(src).is_ok());
}

#[test]
fn coalesce_needs_optional_lhs() {
    let msg = first_error("fn main() -> int32 { let v: int32 = 1 ?? 2; return v; }");
    assert!(msg.contains("`??` needs an `Optional`"), "{msg}");
}

#[test]
fn coalesce_default_must_match_payload() {
    let src = "fn main() -> int32 { let x: string? = None; let v: string = x ?? 1; return 0; }";
    let msg = first_error(src);
    assert!(msg.contains("fallback must be `string`"), "{msg}");
}

#[test]
fn unwrap_or_resolves_on_optional_receiver() {
    let src = "fn main() -> int32 {
let o: int32? = Some(1);
let v: int32 = o.unwrap_or(0);
return v;
}";
    assert!(check_src(src).is_ok());
}

#[test]
fn unwrap_or_rejects_plain_receiver() {
    let msg = first_error("fn main() -> int32 { let n = 5; return n.unwrap_or(0); }");
    assert!(
        msg.contains("expects an `Optional` or `Result` receiver"),
        "{msg}"
    );
}

#[test]
fn unwrap_or_arity_is_enforced() {
    let src = "fn main() -> int32 { let o: int32? = Some(1); return o.unwrap_or(0, 1); }";
    let msg = first_error(src);
    assert!(msg.contains("exactly one argument"), "{msg}");
}

#[test]
fn bare_discard_of_optional_is_a_diagnostic() {
    // §9.9a: symmetric with Result — silent discard hides state.
    let src = "fn f() -> int32? { return None; }
fn main() -> int32 { f(); return 0; }";
    let msg = first_error(src);
    assert!(msg.contains("`_ = expr;`"), "{msg}");
}

#[test]
fn explicit_discard_statement_is_the_escape_hatch() {
    let src = "fn f() -> int32? { return None; }
fn main() -> int32 { _ = f(); return 0; }";
    assert!(check_src(src).is_ok());
}

#[test]
fn closure_literal_with_scalar_capture_types_cleanly() {
    let src = "fn make(prefix: string) -> (int32) -> string {
return fn(v: int32) -> string { return prefix; };
}
fn main() -> int32 { return 0; }";
    assert!(check_src(src).is_ok());
}

#[test]
fn capturing_closure_bearing_struct_is_rejected() {
    // §9.10's exact example shape: n.action stores a closure whose env
    // captures n — the type-level poisoning rule rejects it up front.
    let src = "type Node = { action: () -> unit; }
fn seed() -> () -> unit { return fn() -> unit { return; }; }
fn main() -> int32 {
var n = Node { action: seed() };
n.action = fn() -> unit { n.action(); };
return 0;
}";
    let msg = first_error(src);
    assert!(msg.contains("contains a closure"), "{msg}");
}

#[test]
fn capturing_plain_heap_values_stays_legal() {
    // Negative-of-negative: the poisoning rule must not over-reject.
    let src = "fn greet(name: string) -> () -> string {
return fn() -> string { return name; };
}
fn main() -> int32 { return 0; }";
    assert!(check_src(src).is_ok());
}

#[test]
fn closure_body_must_return_declared_type_shape() {
    let src = "fn make() -> (int32) -> int32 {
return fn(x: int32) -> int32 { };
}
fn main() -> int32 { return 0; }";
    let msg = first_error(src);
    assert!(msg.contains("must end in a value or return"), "{msg}");
}
