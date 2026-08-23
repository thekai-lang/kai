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
}

#[cfg(test)]
mod v0004_tests {
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
        check_with(&merged, &resolution)
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
}

#[cfg(test)]
mod v0005_tests {
    use super::*;
    use crate::test_support::parse_ok;
    use kai_tast::{KaiType, TypedExprKind};

    fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
        let ast = parse_ok(src.as_ref());
        let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
        check_with(&ast, &resolution)
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

    #[test]
    fn string_equality_allowed_and_bool_typed() {
        let src = "fn main() -> int32 { let a = \"x\"; let b = \"y\"; let same = a == b; return 0; }";
        let program = check_src(src).unwrap();
        match &program.fns[0].body.stmts[2] {
            kai_tast::TypedStmt::Let(l) => {
                assert_eq!(l.init.ty, KaiType::Bool);
                assert!(matches!(
                    l.init.kind,
                    TypedExprKind::Binary { op: kai_tast::BinaryOp::Eq, .. }
                ));
            }
            other => panic!("expected let, got {other:?}"),
        }
    }
}
