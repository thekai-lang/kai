//! Parser unit tests — v0.0.5: strings, arrays, for..in.

use super::tests::parse_src;

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