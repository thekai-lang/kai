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
                    assert!(matches!(inner.as_ref(), Ty::Path(n) if n.last().unwrap().name == "string"));
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
                assert!(matches!(ok.as_ref(), Ty::Path(n) if n.last().unwrap().name == "int32"));
                assert!(matches!(err.as_ref(), Ty::Path(n) if n.last().unwrap().name == "string"));
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
                assert!(matches!(ret.as_ref(), Ty::Path(n) if n.last().unwrap().name == "bool"));
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
                assert!(matches!(&clo.ret, Ty::Path(n) if n.last().unwrap().name == "int32"));
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
