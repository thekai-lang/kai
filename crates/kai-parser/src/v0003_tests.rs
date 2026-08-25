//! Parser unit tests — v0.0.3: parameters, types, calls, field access, struct literals.

use super::tests::parse_src;

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