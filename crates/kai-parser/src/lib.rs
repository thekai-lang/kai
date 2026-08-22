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
    let mut parser = Parser::new(tokens);
    let program = decl::program(&mut parser);

    if parser.diagnostics.is_empty() {
        Ok(program)
    } else {
        Err(parser.diagnostics)
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
        }
        assert_eq!(main.body.stmts.len(), 1);
    }

    #[test]
    fn rejects_missing_return_semicolon() {
        let err = parse_src("fn main() -> int32 { return 0 }").unwrap_err();
        assert!(err[0].message.contains("`;`"));
    }

    #[test]
    fn rejects_parameters_in_v001() {
        let err = parse_src("fn f(x) -> int32 { return 0; }").unwrap_err();
        assert!(err[0].message.contains("v0.0.3"));
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
}
