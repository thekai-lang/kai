//! Type checker: untyped AST -> TAST. The only phase allowed to convert
//! surface type names into concrete `KaiType`s.

pub mod decl;
pub mod error;
pub mod expr;
pub mod stmt;
#[cfg(test)]
pub mod test_support;
pub mod ty;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use kai_tast::TypedProgram;

/// Lowers a full program to TAST. Returns every diagnostic found; the TAST is
/// only produced when none occurred, so downstream phases can trust it fully.
pub fn check(program: &Program) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let typed = decl::program(program, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(typed)
    } else {
        Err(diagnostics)
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
    fn rejects_unknown_type() {
        let ast = parse_ok("fn main() -> int64 { return 0; }");
        let diags = check(&ast).unwrap_err();
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown type `int64`"))
        );
    }

    #[test]
    fn rejects_literal_overflow() {
        let ast = parse_ok("fn main() -> int32 { return 2147483648; }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("does not fit in `int32`"));
    }

    #[test]
    fn rejects_missing_return_statement() {
        let ast = parse_ok("fn main() -> int32 { }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("has no `return`"));
    }

    #[test]
    fn rejects_bare_return_in_int32_function() {
        let ast = parse_ok("fn main() -> int32 { return; }");
        let diags = check(&ast).unwrap_err();
        assert!(diags[0].message.contains("must produce a `int32` value"));
    }
}
