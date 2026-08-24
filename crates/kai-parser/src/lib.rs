//! Hand-written recursive-descent parser: tokens -> untyped AST.
//! Errors are diagnostics with spans; parsing never panics on bad input.

pub mod decl;
pub mod error;
pub mod expr;
pub mod parser;
pub mod stmt;
pub mod ty;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use parser::Parser;

/// Parses a full token stream into a `Program`. On any error the diagnostic
/// list is returned instead of a (potentially misleading) tree.
pub fn parse(tokens: &[kai_lexer::Token]) -> Result<Program, Vec<Diagnostic>> {
    // Deeply nested input recurses until the expression budget trips, and
    // debug builds burn kilobytes of native stack per AST level. The budget
    // bounds *work*, not stack depth, so parsing always runs on a dedicated
    // large-stack thread instead of trusting the caller's (rustc does the
    // same for its own passes).
    let owned = tokens.to_vec();
    with_big_stack(move || {
        let mut parser = Parser::new(&owned);
        let program = decl::program(&mut parser);

        if parser.diagnostics.is_empty() {
            Ok(program)
        } else {
            Err(parser.diagnostics)
        }
    })
}

/// Runs `f` on a 64 MiB-stack thread, re-raising any panic unchanged.
/// Shared with the driver pipeline: every phase recurses over user-shaped
/// trees before a budget trips, so all of them need the same headroom.
pub fn with_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    const STACK: usize = 64 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn parser thread");
    match handle.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_lexer::lex;

    fn parse_src(src: &str) -> Result<Program, Vec<Diagnostic>> {
        let out = lex(src);
        assert!(
            out.diagnostics.is_empty(),
            "lexing failed: {:?}",
            out.diagnostics
        );
        parse(&out.tokens)
    }

    #[test]
    fn parses_minimal_program() {
        let program = parse_src("fn main() -> int32 { return 0; }").unwrap();
        assert_eq!(program.fns.len(), 1);
        let main = &program.fns[0];
        assert_eq!(main.name.name, "main");
        match &main.ret {
            kai_ast::Ty::Named(ident) => assert_eq!(ident.name, "int32"),
            other => panic!("expected named type, got {other:?}"),
        }
        assert_eq!(main.body.stmts.len(), 1);
    }

    #[test]
    fn rejects_missing_return_semicolon() {
        let err = parse_src("fn main() -> int32 { return 0 }").unwrap_err();
        assert!(err[0].message.contains("`;`"));
    }

    // -- v0.0.3: parameters, types, calls, field access, struct literals ----

    #[test]
    fn parses_function_parameters() {
        let src = "fn add(a: int32, mut b: int64) -> int64 { return b; }
fn main() -> int32 { return 0; }";
        let program = parse_src(src).unwrap();
        let params = &program.fns[0].params;
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name.name, "a");
        assert!(!params[0].mutable);
        match &params[0].ty {
            kai_ast::Ty::Named(t) => assert_eq!(t.name, "int32"),
            other => panic!("expected named type, got {other:?}"),
        }
        assert_eq!(params[1].name.name, "b");
        assert!(params[1].mutable, "`mut` marks a mutable parameter");
        assert!(program.fns[1].params.is_empty());
    }

    #[test]
    fn parses_type_decl_with_fields() {
        let src = "type Point = { x: int32; y: int32; }
fn main() -> int32 { return 0; }";
        let program = parse_src(src).unwrap();
        assert_eq!(program.types.len(), 1);
        let ty = &program.types[0];
        assert_eq!(ty.name.name, "Point");
        assert_eq!(ty.fields.len(), 2);
        assert_eq!(ty.fields[0].name.name, "x");
        assert_eq!(ty.fields[1].name.name, "y");
    }

    #[test]
    fn parses_use_decls_with_dotted_paths() {
        let src = "use support.math;\nuse std.io;\nfn main() -> int32 { return 0; }";
        let program = parse_src(src).unwrap();
        assert_eq!(program.use_decls.len(), 2);
        let path: Vec<&str> = program.use_decls[0]
            .path
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(path, vec!["support", "math"]);
        assert_eq!(program.use_decls[1].path.len(), 2);
        // Imports precede declarations.
        assert_eq!(program.fns.len(), 1);
    }

    #[test]
    fn import_after_declaration_is_rejected() {
        let src = "fn main() -> int32 { return 0; }\nuse math.extra;";
        let err = parse_src(src).unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message.contains("before all declarations")),
            "expected ordering diagnostic, got {err:?}"
        );
    }

    #[test]
    fn public_flags_parse_on_fn_and_type() {
        let src = "public type Point = { x: int32; }\npublic fn make() -> Point { return Point { x: 1 }; }\nfn main() -> int32 { return 0; }";
        let program = parse_src(src).unwrap();
        assert!(program.types[0].is_public);
        assert!(program.fns[0].is_public);
        assert!(!program.fns[1].is_public);
    }

    #[test]
    fn qualified_struct_literal_head_parses_as_path() {
        let src = "fn main() -> int32 { return math.Point { x: 1 }.x; }";
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Return(Some(e)) => match &e.kind {
                kai_ast::ExprKind::FieldAccess(access) => match &access.base.kind {
                    kai_ast::ExprKind::StructLit(lit) => {
                        let names: Vec<&str> = lit.path.iter().map(|i| i.name.as_str()).collect();
                        assert_eq!(names, vec!["math", "Point"]);
                        assert_eq!(lit.fields.len(), 1);
                    }
                    other => panic!("expected qualified literal base, got {other:?}"),
                },
                other => panic!("expected field access on literal, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn malformed_type_field_terminates_with_diagnostics() {
        // Comma instead of semicolon: recovery must skip the offending
        // token rather than spin forever (regression for an infinite loop).
        let src = "type P = { x: int32, y: int32 }
fn main() -> int32 { return 0; }";
        let err = parse_src(src).unwrap_err();
        assert!(!err.is_empty(), "malformed field must produce diagnostics");
    }

    #[test]
    fn parses_call_statement_and_args() {
        let program = parse_src("fn main() -> unit { print(1, 2 + 3); return; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::Expr(e) => match &e.kind {
                kai_ast::ExprKind::Call(call) => {
                    assert!(matches!(call.callee.kind, kai_ast::ExprKind::Ident(_)));
                    assert_eq!(call.args.len(), 2);
                }
                other => panic!("expected call, got {other:?}"),
            },
            other => panic!("expected expr stmt, got {other:?}"),
        }
    }

    #[test]
    fn parses_field_access_chain() {
        let program = parse_src("fn main() -> int32 { return line.start.x; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::Return(Some(e)) => match &e.kind {
                kai_ast::ExprKind::FieldAccess(outer) => {
                    assert_eq!(outer.field.name, "x");
                    match &outer.base.kind {
                        kai_ast::ExprKind::FieldAccess(inner) => {
                            assert_eq!(inner.field.name, "start");
                        }
                        other => panic!("expected nested access, got {other:?}"),
                    }
                }
                other => panic!("expected field access, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn parses_struct_literal_and_field_place_assignment() {
        let src = "fn main() -> unit {
    var p = Point { x: 1, y: 2 };
    p.x = 10;
    p.y += 5;
    return;
}";
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match &l.init.kind {
                kai_ast::ExprKind::StructLit(slit) => {
                    assert_eq!(slit.path.len(), 1);
                    assert_eq!(slit.path[0].name, "Point");
                    assert_eq!(slit.fields.len(), 2);
                    assert_eq!(slit.fields[0].name.name, "x");
                }
                other => panic!("expected struct literal, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
        match &stmts[1].kind {
            kai_ast::StmtKind::Assign(a) => match &a.target {
                kai_ast::AssignTarget::Path { root, steps } => {
                    assert_eq!(root.name, "p");
                    assert_eq!(steps.len(), 1);
                    let kai_ast::PlaceStep::Field(field) = &steps[0] else {
                        panic!("expected field step");
                    };
                    assert_eq!(field.name, "x");
                }
                other => panic!("expected field place, got {other:?}"),
            },
            other => panic!("expected assign, got {other:?}"),
        }
        assert!(matches!(
            &stmts[2].kind,
            kai_ast::StmtKind::Assign(a) if matches!(&a.target, kai_ast::AssignTarget::Path { .. })
        ));
    }

    #[test]
    fn if_condition_bans_bare_struct_literal() {
        // NO_STRUCT_LITERAL (§9.3): `p == Point { ... }` inside an if reads
        // as `p == Point` followed by a BLOCK — deterministic, never a
        // struct literal.
        let src = "fn main() -> bool {
    if p == Point { return true; }
    return false;
}";
        let program = parse_src(src).unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::If(if_stmt) => match &if_stmt.cond.kind {
                kai_ast::ExprKind::Binary(b) => {
                    assert_eq!(b.op, kai_ast::BinaryOp::Eq);
                    assert!(
                        matches!(&b.rhs.kind, kai_ast::ExprKind::Ident(i) if i.name == "Point")
                    );
                    assert_eq!(if_stmt.then_block.stmts.len(), 1);
                }
                other => panic!("expected comparison cond, got {other:?}"),
            },
            other => panic!("expected if, got {other:?}"),
        }
    }

    #[test]
    fn parenthesized_struct_literal_allowed_in_condition() {
        let src =
            "fn main() -> bool { if (p == Point { x: 1, y: 2 }) { return true; } return false; }";
        let program = parse_src(src).unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::If(if_stmt) => match &if_stmt.cond.kind {
                kai_ast::ExprKind::Binary(b) => {
                    assert!(matches!(&b.rhs.kind, kai_ast::ExprKind::StructLit(_)));
                }
                other => panic!("expected comparison cond, got {other:?}"),
            },
            other => panic!("expected if, got {other:?}"),
        }
    }

    #[test]
    fn non_call_expression_statements_are_rejected() {
        let err = parse_src("fn main() -> unit { 1 + 1; return; }").unwrap_err();
        assert!(
            err[0]
                .message
                .contains("only function calls can appear as expression statements")
        );
    }
    #[test]
    fn accepts_bare_return_grammatically() {
        // Grammar allows `return;`; rejecting it in non-unit functions is
        // the type checker's job.
        let program = parse_src("fn main() -> int32 { return; }").unwrap();
        assert!(matches!(
            program.fns[0].body.stmts[0].kind,
            kai_ast::StmtKind::Return(None)
        ));
    }

    #[test]
    fn parses_let_var_and_assignment() {
        let src = "fn main() -> int32 { let a = 1; var b = a; b = 2; b += 3; return b; }";
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        assert!(matches!(&stmts[0].kind, kai_ast::StmtKind::Let(l) if !l.mutable));
        assert!(matches!(&stmts[1].kind, kai_ast::StmtKind::Let(l) if l.mutable));
        assert!(matches!(
            &stmts[2].kind,
            kai_ast::StmtKind::Assign(a) if a.op == kai_ast::AssignOp::Eq
        ));
        assert!(matches!(
            &stmts[3].kind,
            kai_ast::StmtKind::Assign(a) if a.op == kai_ast::AssignOp::PlusEq
        ));
    }

    #[test]
    fn parses_typed_binding() {
        let program = parse_src("fn main() -> int32 { let x: int64 = 5; return 0; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                kai_ast::Ty::Named(ident) => assert_eq!(ident.name, "int64"),
                other => panic!("expected named type, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_if_else_chain() {
        let src = "fn main() -> int32 { if true { return 1; } else if false { return 2; } else { return 3; } }";
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        assert!(matches!(&stmts[0].kind, kai_ast::StmtKind::If(_)));
    }

    #[test]
    fn precedence_and_binds_tighter_than_or() {
        // `true && false || true` must read as `(true && false) || true`.
        let program = parse_src("fn main() -> bool { return true && false || true; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::Return(Some(e)) => match &e.kind {
                kai_ast::ExprKind::Binary(top) => {
                    assert_eq!(top.op, kai_ast::BinaryOp::Or);
                    assert!(matches!(top.lhs.kind, kai_ast::ExprKind::Binary(_)));
                    assert!(matches!(top.rhs.kind, kai_ast::ExprKind::BoolLit { .. }));
                }
                other => panic!("expected || at top level, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn parses_unary_minus_and_not() {
        let src = "fn main() -> bool { let n = -5; return !(n < 0); }";
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => {
                assert!(matches!(l.init.kind, kai_ast::ExprKind::Unary(_)));
            }
            other => panic!("expected let, got {other:?}"),
        }
        match &stmts[1].kind {
            kai_ast::StmtKind::Return(Some(e)) => {
                assert!(matches!(e.kind, kai_ast::ExprKind::Unary(_)));
            }
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn rejects_non_ident_assign_target() {
        let err = parse_src("fn main() -> int32 { 1 + 1 = 2; return 0; }").unwrap_err();
        assert!(err[0].message.contains("invalid assignment target"));
    }

    #[test]
    fn parenthesized_expression_parses() {
        let program = parse_src("fn main() -> int32 { return (1 + 2) * 3; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::Return(Some(e)) => match &e.kind {
                kai_ast::ExprKind::Binary(b) => assert_eq!(b.op, kai_ast::BinaryOp::Mul),
                other => panic!("expected mul at top level, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    // -- recursion budget ---------------------------------------------------

    fn deep_parens(depth: usize) -> String {
        let mut src = String::from("fn main() -> int32 { return ");
        src.push_str(&"(".repeat(depth));
        src.push('1');
        src.push_str(&")".repeat(depth));
        src.push_str("; }");
        src
    }

    #[test]
    fn deep_parens_fail_cleanly_without_stack_overflow() {
        // 10k nesting used to overflow the native stack; now it is a budget
        // diagnostic, never a crash.
        let err = parse_src(&deep_parens(10_000)).unwrap_err();
        assert!(err[0].message.contains("nested too deeply"));
    }

    #[test]
    fn moderate_nesting_still_parses() {
        parse_src(&deep_parens(100)).expect("100 levels are within budget");
    }

    #[test]
    fn deep_unary_chain_is_budgeted_too() {
        // Iterative parsing must not become an unbounded-AST loophole:
        // downstream phases recurse over operator chains as well.
        let mut src = String::from("fn main() -> bool { return ");
        src.push_str(&"-".repeat(1_000));
        src.push_str("1; }");
        let err = parse_src(&src).unwrap_err();
        assert!(err[0].message.contains("nested too deeply"));
    }

    #[test]
    fn small_unary_chain_parses() {
        let program = parse_src("fn main() -> int32 { return - - 5; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            kai_ast::StmtKind::Return(Some(e)) => {
                assert!(matches!(e.kind, kai_ast::ExprKind::Unary(_)));
            }
            other => panic!("expected unary, got {other:?}"),
        }
    }

    #[test]
    fn depth_budget_reports_each_independent_overflow() {
        // The reported flag resets per recovery region: two separate deep
        // expressions yield two diagnostics, not one.
        let mut src = String::from("fn main() -> int32 { return ");
        src.push_str(&"(".repeat(1_000));
        src.push('1');
        src.push_str(&")".repeat(1_000));
        src.push_str("; return ");
        src.push_str(&"(".repeat(1_000));
        src.push('2');
        src.push_str(&")".repeat(1_000));
        src.push_str("; }");
        let err = parse_src(&src).unwrap_err();
        assert_eq!(err.len(), 2);
        assert!(err.iter().all(|d| d.message.contains("nested too deeply")));
    }

    #[test]
    fn budget_recovery_preserves_surrounding_statements() {
        // After skipping the over-deep group, later statements still parse
        // (internal parser used here so the recovered tree can be inspected;
        // run on the shared big stack like `parse` itself).
        with_big_stack(move || {
            let mut src = String::from("fn main() -> int32 { return ");
            src.push_str(&"(".repeat(1_000));
            src.push('1');
            src.push_str(&")".repeat(1_000));
            src.push_str("; return 42; }");

            let out = kai_lexer::lex(&src);
            let mut p = parser::Parser::new(&out.tokens);
            let program = decl::program(&mut p);

            assert_eq!(p.diagnostics.len(), 1);
            assert!(p.diagnostics[0].message.contains("nested too deeply"));
            assert_eq!(program.fns[0].body.stmts.len(), 2, "both returns survive");
        })
    }
    // -- v0.0.5: strings, arrays, for..in ------------------------------------

    #[test]
    fn parses_string_literals_with_escapes() {
        let src = r#"fn main() -> int32 { let s = "a\nb\t\"q\\"; return 0; }"#;
        let program = parse_src(src).unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match &l.init.kind {
                kai_ast::ExprKind::StrLit(text) => assert_eq!(text.value, "a\nb\t\"q\\"),
                other => panic!("expected string literal, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_array_literal_and_indexing() {
        let program =
            parse_src("fn main() -> int32 { let a = [1, 2, 3]; return a[1]; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match &l.init.kind {
                kai_ast::ExprKind::ArrayLit(lit) => assert_eq!(lit.elements.len(), 3),
                other => panic!("expected array literal, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
        match &stmts[1].kind {
            kai_ast::StmtKind::Return(Some(e)) => match &e.kind {
                kai_ast::ExprKind::Index(ix) => {
                    assert!(matches!(ix.base.kind, kai_ast::ExprKind::Ident(_)));
                    assert!(matches!(ix.index.kind, kai_ast::ExprKind::IntLit(_)));
                }
                other => panic!("expected index expr, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn parses_empty_array_literal() {
        // `[]` parses fine; requiring context typing is a typecheck rule.
        let program = parse_src("fn main() -> int32 { let a = []; return 0; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => {
                assert!(matches!(l.init.kind, kai_ast::ExprKind::ArrayLit(_)))
            }
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_array_type_annotation() {
        let program =
            parse_src("fn main() -> int32 { let a: int64[] = [1]; return 0; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                kai_ast::Ty::Array(elem) => match elem.as_ref() {
                    kai_ast::Ty::Named(t) => assert_eq!(t.name, "int64"),
                    other => panic!("named element expected, got {other:?}"),
                },
                other => panic!("array type expected, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_indexed_place_assignment() {
        let program =
            parse_src("fn main() -> int32 { var a = [1]; a[0] = 2; return 0; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[1].kind {
            kai_ast::StmtKind::Assign(a) => match &a.target {
                kai_ast::AssignTarget::Path { root, steps } => {
                    assert_eq!(root.name, "a");
                    assert_eq!(steps.len(), 1);
                    assert!(matches!(&steps[0], kai_ast::PlaceStep::Index { .. }));
                }
                other => panic!("expected path place, got {other:?}"),
            },
            other => panic!("expected assign, got {other:?}"),
        }
    }

    #[test]
    fn parses_mixed_field_index_place() {
        let program = parse_src(
            "fn main() -> int32 { var p = q(); p.arr[0] = 2; return 0; }
fn q() -> Point { return Point { x: 1 }; }
type Point = { x: int32; }",
        )
        .unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[1].kind {
            kai_ast::StmtKind::Assign(a) => match &a.target {
                kai_ast::AssignTarget::Path { root, steps } => {
                    assert_eq!(root.name, "p");
                    assert_eq!(steps.len(), 2);
                    assert!(matches!(&steps[0], kai_ast::PlaceStep::Field(_)));
                    assert!(matches!(&steps[1], kai_ast::PlaceStep::Index { .. }));
                }
                other => panic!("expected path place, got {other:?}"),
            },
            other => panic!("expected assign, got {other:?}"),
        }
    }

    #[test]
    fn parses_for_in_loop() {
        let program = parse_src(
            "fn main() -> int32 { let a = [1, 2]; for x in a { print(x); } return 0; }
fn print(v: int32) -> unit { return; }",
        )
        .unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[1].kind {
            kai_ast::StmtKind::For(f) => {
                assert_eq!(f.binding.name, "x");
                assert!(matches!(f.iterable.kind, kai_ast::ExprKind::Ident(_)));
                assert_eq!(f.body.stmts.len(), 1);
            }
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn for_iterable_does_not_swallow_body_brace() {
        // NO_STRUCT_LITERAL extends to the iterable position (§9.3): in
        // `for c in chars {`, the `{` is the loop body — never a struct
        // literal opener. The body block must survive parsing.
        let program = parse_src(
            "fn main() -> int32 { for c in chars { return 0; } return 0; }",
        )
        .unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::For(f) => assert_eq!(f.body.stmts.len(), 1),
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn string_in_array_literal_position() {
        let program =
            parse_src(r#"fn main() -> int32 { let a = ["x", "y"]; return 0; }"#).unwrap();
        let stmts = &program.fns[0].body.stmts;
        match &stmts[0].kind {
            kai_ast::StmtKind::Let(l) => match &l.init.kind {
                kai_ast::ExprKind::ArrayLit(lit) => assert_eq!(lit.elements.len(), 2),
                other => panic!("expected array literal, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }
}

// -- v0.0.6: Optional/Result/closures/discard ---------------------------------

#[cfg(test)]
mod v0006_tests {
    use super::*;
    use kai_ast::{ExprKind, StmtKind, Ty};

    fn parse_src(src: &str) -> Result<Program, Vec<Diagnostic>> {
        let out = kai_lexer::lex(src);
        assert!(
            out.diagnostics.is_empty(),
            "lexing failed: {:?}",
            out.diagnostics
        );
        parse(&out.tokens)
    }

    #[test]
    fn optional_type_both_spellings_one_form() {
        // `T?` desugars straight to Optional<T> at parse time (§9.9a) —
        // both source spellings yield the identical AST.
        for src in [
            "fn main() -> int32 { let x: string? = None; return 0; }",
            "fn main() -> int32 { let x: Optional<string> = None; return 0; }",
        ] {
            let program = parse_src(src).unwrap();
            match &program.fns[0].body.stmts[0].kind {
                StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                    Ty::Optional(inner) => {
                        assert!(matches!(inner.as_ref(), Ty::Named(n) if n.name == "string"));
                    }
                    other => panic!("expected optional type, got {other:?}"),
                },
                other => panic!("expected let, got {other:?}"),
            }
        }
    }

    #[test]
    fn sugar_applies_after_array_suffix() {
        // `string[]?` is Optional<string[]> — the `?` follows the brackets.
        let program =
            parse_src("fn main() -> int32 { let x: string[]? = None; return 0; }").unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                Ty::Optional(inner) => assert!(matches!(inner.as_ref(), Ty::Array(_))),
                other => panic!("expected optional-of-array, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_result_type() {
        let program = parse_src(
            "fn main() -> int32 { let r: Result<int32, string> = q(); return 0; }
fn q() -> Result<int32, string> { return Result { }; }",
        )
        .unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                Ty::Result { ok, err } => {
                    assert!(matches!(ok.as_ref(), Ty::Named(n) if n.name == "int32"));
                    assert!(matches!(err.as_ref(), Ty::Named(n) if n.name == "string"));
                }
                other => panic!("expected result type, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn parses_closure_type() {
        let program = parse_src(
            "fn main() -> int32 { let f: (int32, string) -> bool = q(); return 0; }
fn q() -> (int32, string) -> bool { return f; }",
        )
        .unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Let(l) => match l.ty.as_ref().expect("annotation") {
                Ty::Closure { params, ret } => {
                    assert_eq!(params.len(), 2);
                    assert!(matches!(ret.as_ref(), Ty::Named(n) if n.name == "bool"));
                }
                other => panic!("expected closure type, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
    }

    #[test]
    fn unknown_generic_is_rejected() {
        // Builtin-only parametric machinery: only Optional/Result take `<T>`.
        let err = parse_src(
            "type Box = { v: int32; }
fn main() -> int32 { let b: Box<int32> = q(); return 0; }
fn q() -> Box<int32> { return q(); }",
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message.contains("cannot take type parameters")),
            "got {err:?}"
        );
    }

    #[test]
    fn wrong_arity_optional_is_rejected() {
        let err = parse_src(
            "fn main() -> int32 { let x: Optional<int32, int32> = None; return 0; }",
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message.contains("exactly one type parameter")),
            "got {err:?}"
        );
    }

    #[test]
    fn parses_some_and_none_literals() {
        let program =
            parse_src("fn main() -> int32 { let a = Some(1); let b = None; return 0; }").unwrap();
        let stmts = &program.fns[0].body.stmts;
        assert!(matches!(
            &stmts[0].kind,
            StmtKind::Let(l) if matches!(&l.init.kind, ExprKind::SomeLit(_))
        ));
        assert!(matches!(
            &stmts[1].kind,
            StmtKind::Let(l) if matches!(&l.init.kind, ExprKind::NoneLit)
        ));
    }

    #[test]
    fn coalesce_is_right_associative() {
        let program =
            parse_src("fn main() -> int32 { return a ?? b ?? c; }
fn a() -> int32 { return 0; }
fn b() -> int32 { return 0; }
fn c() -> int32 { return 0; }")
        .unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Return(Some(e)) => match &e.kind {
                ExprKind::Coalesce(top) => {
                    assert!(matches!(&top.lhs.kind, ExprKind::Ident(i) if i.name == "a"));
                    match &top.rhs.kind {
                        ExprKind::Coalesce(inner) => {
                            assert!(matches!(&inner.lhs.kind, ExprKind::Ident(i) if i.name == "b"));
                            assert!(matches!(&inner.rhs.kind, ExprKind::Ident(i) if i.name == "c"));
                        }
                        other => panic!("expected nested coalesce, got {other:?}"),
                    }
                }
                other => panic!("expected coalesce at top, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn unwrap_or_parses_as_ordinary_call_composition() {
        // Decision recorded in the EBNF: `.unwrap_or` has NO dedicated
        // production — it composes from FieldAccess + Call exactly like any
        // method-shaped call; builtin status is a typecheck question.
        let program =
            parse_src("fn main() -> int32 { return x.unwrap_or(0); }
fn x() -> int32 { return 0; }")
        .unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Return(Some(e)) => match &e.kind {
                ExprKind::Call(call) => match &call.callee.kind {
                    ExprKind::FieldAccess(access) => {
                        assert_eq!(access.field.name, "unwrap_or");
                        assert_eq!(call.args.len(), 1);
                    }
                    other => panic!("expected field-access callee, got {other:?}"),
                },
                other => panic!("expected call, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn parses_catch_expression_with_statements_and_tail() {
        let program = parse_src(
            "fn log(e: string) -> unit { return; }
fn main() -> int32 {
    var count = 0;
    let r = q() catch |err| { log(err); count = count + 1; 0 };
    return r;
}
fn q() -> int32 { return 0; }",
        )
        .unwrap();
        let init = match &program.fns[1].body.stmts[1].kind {
            StmtKind::Let(l) => &l.init,
            other => panic!("expected let, got {other:?}"),
        };
        match &init.kind {
            ExprKind::Catch(c) => {
                assert_eq!(c.err_binding.name, "err");
                // `log(err);` and `count = count + 1;` are statements;
                // `0` is the mandatory tail.
                assert_eq!(c.stmts.len(), 2);
                assert!(matches!(&c.tail.kind, ExprKind::IntLit(v) if v.value == 0));
            }
            other => panic!("expected catch expression, got {other:?}"),
        }
    }

    #[test]
    fn catch_block_requires_trailing_value() {
        // CatchBlock ::= '{' { Stmt } Expr '}' — `{ f(); }` has no tail.
        let err = parse_src(
            "fn f() -> unit { return; }
fn main() -> int32 { let r = q() catch |e| { f(); }; return r; }
fn q() -> int32 { return 0; }",
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message.contains("must end with a value expression")),
            "got {err:?}"
        );
    }

    #[test]
    fn parses_discard_statement() {
        let program = parse_src(
            "fn f() -> unit { return; }
fn main() -> unit { _ = f(); _ = 1 + 2; return; }",
        )
        .unwrap();
        let stmts = &program.fns[1].body.stmts;
        assert!(matches!(&stmts[0].kind, StmtKind::Discard(_)));
        assert!(matches!(&stmts[1].kind, StmtKind::Discard(_)));
    }

    #[test]
    fn let_underscore_is_parse_level_rejection() {
        // §9.9b: `_` is not an Ident, so `let _ = ...` never reaches typecheck.
        let err = parse_src("fn main() -> int32 { let _ = 1; return 0; }").unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message.contains("a variable name")),
            "got {err:?}"
        );
    }

    #[test]
    fn stray_pipe_in_operator_position_is_rejected_with_hint() {
        let err = parse_src("fn main() -> int32 { return a | b; }
fn a() -> int32 { return 0; }
fn b() -> int32 { return 0; }")
        .unwrap_err();
        assert!(
            err.iter().any(|d| d.message.contains("`|` is not an operator")),
            "got {err:?}"
        );
    }

    #[test]
    fn parses_closure_literal() {
        let program = parse_src(
            "fn make() -> (int32) -> int32 {
    return fn(x: int32) -> int32 { return x + 1; };
}
fn main() -> int32 { return 0; }",
        )
        .unwrap();
        match &program.fns[0].body.stmts[0].kind {
            StmtKind::Return(Some(e)) => match &e.kind {
                ExprKind::ClosureLit(clo) => {
                    assert_eq!(clo.params.len(), 1);
                    assert_eq!(clo.body.stmts.len(), 1);
                    assert!(matches!(&clo.ret, Ty::Named(n) if n.name == "int32"));
                }
                other => panic!("expected closure literal, got {other:?}"),
            },
            other => panic!("expected return, got {other:?}"),
        }
    }

    #[test]
    fn closure_body_return_stays_inside_the_literal() {
        // The literal's block consumes its own `}`; parsing continues in the
        // enclosing statement stream afterwards.
        let program = parse_src(
            "fn main() -> int32 {
    let f = fn(x: int32) -> int32 { return x; };
    return f(1);
}",
        )
        .unwrap();
        let stmts = &program.fns[0].body.stmts;
        assert!(matches!(&stmts[0].kind, StmtKind::Let(l) if matches!(&l.init.kind, ExprKind::ClosureLit(_))));
        assert!(matches!(&stmts[1].kind, StmtKind::Return(Some(_))));
    }
}
