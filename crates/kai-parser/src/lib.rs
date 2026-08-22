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
}
