//! Entry-point resolution for v0.0.1:
//! - exactly one `main`
//! - no duplicate top-level function names
//! - `main` must be `() -> int32`

use kai_ast::{Program, Ty};
use kai_diagnostics::{Diagnostic, Span};

pub fn check_entry(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_duplicate_names(program, &mut diagnostics);
    check_main(program, &mut diagnostics);

    diagnostics
}

fn check_duplicate_names(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    for (idx, decl) in program.fns.iter().enumerate() {
        if program.fns[..idx]
            .iter()
            .any(|other| other.name.name == decl.name.name)
        {
            diagnostics.push(Diagnostic::error(
                format!("duplicate function `{}`", decl.name.name),
                decl.name.span,
            ));
        }
    }
}

fn check_main(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let mains: Vec<&kai_ast::FnDecl> = program
        .fns
        .iter()
        .filter(|decl| decl.name.name == "main")
        .collect();

    match mains.as_slice() {
        [] => {
            let span = program.fns.first().map_or(Span::new(0, 0), |f| f.span);
            diagnostics.push(Diagnostic::error("program has no `main` function", span));
        }
        [main] => {
            if !main.params.is_empty() {
                diagnostics.push(Diagnostic::error(
                    "`main` must take no parameters",
                    main.name.span,
                ));
            }
            if !returns_int32(main) {
                diagnostics.push(Diagnostic::error(
                    "`main` must return `int32`",
                    main.ret.span(),
                ));
            }
        }
        // Multiple mains: already reported by `check_duplicate_names`.
        [..] => {}
    }
}

/// v0.0.1 compares the surface name; the type checker re-validates against
/// resolved types once aliases exist.
fn returns_int32(decl: &kai_ast::FnDecl) -> bool {
    matches!(&decl.ret, Ty::Named(ident) if ident.name == "int32" || ident.name == "int")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_ast::{Block, FnDecl, Ident, Param, Stmt, StmtKind};
    use kai_diagnostics::Span;

    fn named(name: &str) -> Ty {
        Ty::Named(Ident {
            name: name.into(),
            span: Span::new(0, 0),
        })
    }

    fn decl(name: &str, ret: Ty) -> FnDecl {
        FnDecl {
            name: Ident {
                name: name.into(),
                span: Span::new(0, 0),
            },
            params: Vec::<Param>::new(),
            ret,
            body: Block {
                stmts: vec![Stmt {
                    kind: StmtKind::Return(None),
                    span: Span::new(0, 0),
                }],
                span: Span::new(0, 0),
            },
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn accepts_valid_main() {
        let program = Program {
            fns: vec![decl("main", named("int32"))],
        };
        assert!(check_entry(&program).is_empty());
    }

    #[test]
    fn accepts_int_alias_for_main() {
        let program = Program {
            fns: vec![decl("main", named("int"))],
        };
        assert!(check_entry(&program).is_empty());
    }

    #[test]
    fn rejects_missing_main() {
        let program = Program {
            fns: vec![decl("foo", named("int32"))],
        };
        assert_eq!(
            check_entry(&program)[0].message,
            "program has no `main` function"
        );
    }

    #[test]
    fn rejects_wrong_main_return_type() {
        let program = Program {
            fns: vec![decl("main", named("bool"))],
        };
        assert_eq!(check_entry(&program).len(), 1);
        assert!(check_entry(&program)[0].message.contains("`int32`"));
    }

    #[test]
    fn rejects_duplicate_names() {
        let program = Program {
            fns: vec![decl("main", named("int32")), decl("main", named("int32"))],
        };
        assert_eq!(check_entry(&program).len(), 1);
    }
}
