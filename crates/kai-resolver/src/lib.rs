//! Name resolution over the untyped AST.
//!
//! v0.0.3 scope:
//! - separate namespaces for types and functions (Rust-style): the same name
//!   may denote a struct and a function at once; duplicates are an error
//!   within a namespace only
//! - a table of declared structs (name -> declaration index) and functions
//! - cyclic struct definitions are a compile error, reported as a cycle path
//! - the entry-point contract (`main`, no params, returns int32)
//!
//! The resolver never mutates the AST and knows nothing about type semantics
//! — unknown type *names* in annotations are the type checker's diagnostics.

pub mod entry;
pub mod tables;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;

pub use entry::check_entry;
pub use tables::Resolution;

/// Resolves names and validates top-level structure. On success the returned
/// `Resolution` feeds the type checker; on failure the diagnostic list is
/// complete for this phase.
pub fn analyze(program: &Program) -> Result<Resolution, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    let resolution = tables::build(program, &mut diagnostics);
    tables::detect_cycles(program, &resolution.types, &mut diagnostics);
    check_entry(program, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(resolution)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_ast::{Block, FieldDecl, FnDecl, Ident, Param, Stmt, StmtKind, Ty, TypeDecl};
    use kai_diagnostics::Span;

    fn ident(name: &str) -> Ident {
        Ident {
            name: name.into(),
            span: Span::new(0, 0),
        }
    }

    fn named(name: &str) -> Ty {
        Ty::Named(ident(name))
    }

    fn decl(name: &str, ret: Ty) -> FnDecl {
        FnDecl {
            is_public: false,
            name: ident(name),
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

    fn type_decl(name: &str, fields: Vec<(&str, &str)>) -> TypeDecl {
        TypeDecl {
            is_public: false,
            name: ident(name),
            fields: fields
                .into_iter()
                .map(|(fname, fty)| FieldDecl {
                    name: ident(fname),
                    ty: named(fty),
                })
                .collect(),
            span: Span::new(0, 0),
        }
    }

    fn analyze_of(program: &Program) -> Result<Resolution, Vec<Diagnostic>> {
        analyze(program)
    }

    #[test]
    fn builds_separate_namespaces() {
        // Rust-style: a struct and a function may share one name.
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32")), decl("Point", named("int32"))],
            types: vec![type_decl("Point", vec![("x", "int32")])],
        };
        let resolution = analyze_of(&program).unwrap();
        assert_eq!(resolution.types["Point"], 0);
        assert_eq!(resolution.fns["Point"], 1);
        assert_eq!(resolution.fns["main"], 0);
    }

    #[test]
    fn rejects_duplicate_types() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![
                type_decl("Point", vec![("x", "int32")]),
                type_decl("Point", vec![("y", "int32")]),
            ],
        };
        assert!(
            analyze_of(&program).unwrap_err()[0]
                .message
                .contains("duplicate type `Point`")
        );
    }

    #[test]
    fn rejects_duplicate_fields_in_one_type() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![type_decl("P", vec![("x", "int32"), ("x", "int64")])],
        };
        assert!(
            analyze_of(&program).unwrap_err()[0]
                .message
                .contains("duplicate field `x` in type `P`")
        );
    }

    #[test]
    fn detects_direct_self_cycle() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![type_decl("A", vec![("next", "A")])],
        };
        assert!(
            analyze_of(&program).unwrap_err()[0]
                .message
                .contains("cyclic type: A -> A")
        );
    }

    #[test]
    fn detects_two_node_cycle_with_path() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![
                type_decl("A", vec![("b", "B")]),
                type_decl("B", vec![("a", "A")]),
            ],
        };
        assert!(
            analyze_of(&program).unwrap_err()[0]
                .message
                .contains("cyclic type: A -> B -> A")
        );
    }

    #[test]
    fn unknown_field_type_is_not_a_cycle_edge() {
        // `Foo` is undeclared; the cycle check must not treat it as a
        // self-edge or crash. The unknown name is reported by the checker.
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![type_decl("A", vec![("f", "Foo")])],
        };
        assert!(analyze_of(&program).is_ok());
    }

    #[test]
    fn acyclic_chain_is_accepted() {
        let program = Program {
            use_decls: Vec::new(),
            fns: vec![decl("main", named("int32"))],
            types: vec![
                type_decl("Inner", vec![("v", "int32")]),
                type_decl("Outer", vec![("inner", "Inner")]),
            ],
        };
        assert!(analyze_of(&program).is_ok());
    }
}
