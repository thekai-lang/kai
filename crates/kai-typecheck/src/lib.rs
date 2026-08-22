//! Type checker: untyped AST -> TAST. The only phase allowed to convert
//! surface type names into concrete `KaiType`s, resolve variable names, and
//! enforce mutability (§9.3).

mod checker;
pub mod decl;
pub mod error;
pub mod expr;
mod scope;
pub mod stmt;
#[cfg(test)]
pub mod test_support;
pub mod ty;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use kai_resolver::Resolution;
use kai_tast::TypedProgram;

/// Typecheck-only entry (unit tests / tools): no name resolution performed,
/// so surface names cannot be resolved beyond primitives. The pipeline uses
/// `analyze` + `check_with`.
pub fn check(program: &Program) -> Result<TypedProgram, Vec<Diagnostic>> {
    check_with(program, &Resolution::default())
}

/// Lowers a full program to TAST using the resolver's name tables. Returns
/// every diagnostic found; the TAST is only produced when none occurred, so
/// downstream phases can trust it fully.
pub fn check_with(
    program: &Program,
    resolution: &Resolution,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut state = checker::Checker::new(resolution);
    let typed = decl::program(&mut state, program);

    if !state.failed() {
        Ok(typed)
    } else {
        Err(state.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MAIN_OK, parse_ok};

    #[test]
    fn produces_typed_main() {
        let ast = parse_ok(MAIN_OK);
        let tast = check(&ast).unwrap();
        assert_eq!(tast.fns.len(), 1);
        assert_eq!(tast.fns[0].name, "main");
        assert_eq!(tast.fns[0].ret, kai_tast::KaiType::Int32);
    }

    #[test]
    fn accepts_widened_types_in_v002() {
        // int64 / float64 / bool are legal since v0.0.2; only `main` is
        // constrained to return int32 (resolver's job).
        let ast = parse_ok("fn f() -> int64 { return 0; }");
        assert!(check(&ast).is_ok());

        let ast = parse_ok("fn g() -> float64 { return 1.5; }");
        assert!(check(&ast).is_ok());
    }

    #[test]
    fn rejects_unknown_type() {
        let ast = parse_ok("fn main() -> float32 { return 0.0; }");
        let diags = check(&ast).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown type `float32`"))
        );
    }

    #[test]
    fn rejects_literal_overflow() {
        let ast = parse_ok("fn main() -> int32 { return 2147483648; }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("does not fit in `int32`"));
    }

    #[test]
    fn literal_fits_when_annotated_int64() {
        let ast = parse_ok("fn main() -> int64 { let x: int64 = 2147483648; return x; }");
        assert!(check(&ast).is_ok());
    }

    #[test]
    fn rejects_missing_return_statement() {
        let ast = parse_ok("fn main() -> int32 { }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("has no `return`"));
    }

    #[test]
    fn if_else_both_returning_satisfies_definite_return() {
        let src = "fn sign() -> int32 { if true { return 1; } else { return -1; } }";
        let ast = parse_ok(src);
        assert!(check(&ast).is_ok());
    }

    #[test]
    fn if_without_else_does_not_satisfy_definite_return() {
        let src = "fn f() -> int32 { if true { return 1; } }";
        let ast = parse_ok(src);
        let diags = check(&ast).unwrap_err();
        assert!(diags.iter().any(|d| d.message.contains("has no `return`")));
    }

    #[test]
    fn rejects_bare_return_in_int32_function() {
        let ast = parse_ok("fn main() -> int32 { return; }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("must produce a `int32` value"));
    }

    #[test]
    fn assignment_to_let_binding_is_rejected() {
        let src = "fn main() -> int32 { let x = 1; x = 2; return x; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cannot assign to `x`"))
        );
    }

    #[test]
    fn assignment_to_var_binding_is_accepted() {
        let src = "fn main() -> int32 { var x = 1; x = 2; x += 3; return x; }";
        assert!(check(&parse_ok(src)).is_ok());
    }

    #[test]
    fn undeclared_variable_rejected() {
        let src = "fn main() -> int32 { return n; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("undeclared variable `n`"))
        );
    }

    #[test]
    fn same_scope_redeclaration_rejected_but_shadowing_allowed() {
        let bad = "fn main() -> int32 { let x = 1; let x = 2; return x; }";
        let diags = check(&parse_ok(bad)).unwrap_err();
        assert!(diags.iter().any(|d| d.message.contains("already declared")));

        let good = "fn main() -> int32 { let x = 1; { let x: float64 = 1.5; } return x; }";
        assert!(check(&parse_ok(good)).is_ok());
    }

    #[test]
    fn condition_must_be_bool() {
        let src = "fn main() -> int32 { if 1 { return 2; } return 0; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("condition must be `bool`"))
        );
    }

    #[test]
    fn mixed_arithmetic_rejected() {
        let src = "fn main() -> int32 { return 1 + 2.5; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("requires equal types"))
        );
    }

    #[test]
    fn modulo_requires_integers() {
        let src = "fn main() -> float64 { return 4.0 % 2.0; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("%` requires integer"))
        );
    }

    #[test]
    fn comparisons_yield_bool_and_chain_via_parens() {
        let src = "fn main() -> bool { return (1 < 2) == (3 >= 3); }";
        assert!(check(&parse_ok(src)).is_ok());
    }

    #[test]
    fn logical_operators_require_bools() {
        let src = "fn main() -> bool { return 1 && true; }";
        let diags = check(&parse_ok(src)).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("`&&` cannot be applied"))
        );
    }

    #[test]
    fn not_requires_bool_neg_requires_numeric() {
        let a = "fn main() -> bool { return !5; }";
        assert!(
            check(&parse_ok(a))
                .unwrap_err()
                .iter()
                .any(|d| d.message.contains("`!` cannot be applied"))
        );

        let b = "fn main() -> bool { return -true; }";
        assert!(
            check(&parse_ok(b))
                .unwrap_err()
                .iter()
                .any(|d| d.message.contains("`-` cannot be applied"))
        );
    }

    #[test]
    fn negated_min_i32_is_accepted_as_int32() {
        // 2147483648 alone overflows, but its negation fits int32 exactly.
        let src = "fn main() -> int32 { return -2147483648; }";
        assert!(check(&parse_ok(src)).is_ok());
    }

    #[test]
    fn local_ids_are_sequential_per_function() {
        let src =
            "fn main() -> int32 { let a = 1; var b = 2; { let c = 3; b = c + a; } return b; }";
        let tast = check(&parse_ok(src)).unwrap();
        let mut ids = Vec::new();
        collect_locals(&tast.fns[0].body, &mut ids);
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    fn collect_locals(block: &kai_tast::TypedBlock, out: &mut Vec<u32>) {
        for stmt in &block.stmts {
            match stmt {
                kai_tast::TypedStmt::Let(l) => out.push(l.local.0),
                kai_tast::TypedStmt::If(i) => {
                    collect_locals(&i.then_block, out);
                    if let Some(e) = &i.else_block {
                        collect_locals(e, out);
                    }
                }
                kai_tast::TypedStmt::Block(b) => collect_locals(b, out),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod v0003_tests {
    use super::*;
    use crate::test_support::parse_ok;

    /// Full pipeline up to (and including) typecheck: resolution included,
    /// which struct tests need for name tables.
    fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
        let src = src.as_ref();
        let ast = parse_ok(src);
        let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
        check_with(&ast, &resolution)
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
                .any(|d| d.message.contains("use `var`"))
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
}
