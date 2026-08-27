//! Entry-point contract: exactly one `main`, no parameters, returns int32,
//! living in the ENTRY module (module 0). An imported public `main` does not
//! make a program runnable. (Duplicate top-level names live in `tables`.)

use kai_ast::{Program, Ty};
use kai_diagnostics::{Diagnostic, Span};

use crate::tables::Resolution;

pub fn check_entry(
    program: &Program,
    resolution: &Resolution,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_main(program, resolution, diagnostics);
}

fn check_main(
    program: &Program,
    resolution: &Resolution,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mains: Vec<&kai_ast::FnDecl> = program
        .fns
        .iter()
        .enumerate()
        // Ownership filter: only the entry module can provide `main`.
        .filter(|(idx, decl)| {
            decl.name.name == "main"
                && resolution.fn_module.get(*idx).copied() == Some(0)
        })
        .map(|(_, decl)| decl)
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
            is_public: false,
            name: Ident {
                name: name.into(),
                span: Span::new(0, 0),
            },
            params: Vec::<Param>::new(),
            ret,
            effects: None,
            is_reversible: false,
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

    fn diags_of(program: &Program) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let resolution = crate::tables::build_single(program, &mut Vec::new());
        check_entry(program, &resolution, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn accepts_valid_main() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: Vec::new(),
        };
        assert!(diags_of(&program).is_empty());
    }

    #[test]
    fn accepts_int_alias_for_main() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int"))],
            types: Vec::new(),
        };
        assert!(diags_of(&program).is_empty());
    }

    #[test]
    fn rejects_missing_main() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("foo", named("int32"))],
            types: Vec::new(),
        };
        assert_eq!(
            diags_of(&program)[0].message,
            "program has no `main` function"
        );
    }

    #[test]
    fn rejects_wrong_main_return_type() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("bool"))],
            types: Vec::new(),
        };
        let diags = diags_of(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`int32`"));
    }

    #[test]
    fn rejects_main_with_parameters() {
        let mut main = decl("main", named("int32"));
        main.params.push(Param {
            name: Ident {
                name: "x".into(),
                span: Span::new(0, 0),
            },
            ty: named("int32"),
            mutable: false,
        });
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![main],
            types: Vec::new(),
        };
        assert!(
            diags_of(&program)[0]
                .message
                .contains("must take no parameters")
        );
    }
}
