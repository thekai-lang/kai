//! Pipeline orchestration:
//! lex -> parse -> resolve -> typecheck -> ownership -> codegen.
//! Each phase runs only after the previous one produced no diagnostics; the
//! first failing phase's diagnostics are reported and compilation stops.
//!
//! Two front doors:
//! - `compile`/`jit` take source text (single anonymous module; `use` is a
//!   diagnostic, since imports need a project root)
//! - `compile_file`/`jit_file` take an ENTRY FILE and load its whole module
//!   tree from the project root (§3.6)

use std::path::Path;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use kai_resolver::ModuleInput;
use kai_tast::TypedProgram;

#[derive(Debug, Clone)]
pub struct Failure {
    pub phase: &'static str,
    pub diagnostics: Vec<Diagnostic>,
    /// (display path, source) for every module in play — lets the reporter
    /// render carets for diagnostics attributed to any file. Empty for the
    /// string API, whose caller already holds the single source.
    pub sources: Vec<(String, String)>,
}

/// Full compile to textual LLVM IR (module verified).
pub fn compile(source: &str) -> Result<String, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::compile_ir("kai_module", &program).map_err(internal_failure)
    })
}

/// JIT-compile and execute `main`, returning its `int32` result.
pub fn jit(source: &str) -> Result<i32, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::run_jit(&program).map_err(internal_failure)
    })
}

/// Compiles the module tree rooted at `entry` to textual LLVM IR.
pub fn compile_file(entry: &Path) -> Result<String, Failure> {
    let entry = entry.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let program = lower_modules(&modules)?;
        kai_codegen::compile_ir("kai_module", &program).map_err(internal_failure)
    })
}

/// JIT-executes the module tree rooted at `entry`.
pub fn jit_file(entry: &Path) -> Result<i32, Failure> {
    let entry = entry.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let program = lower_modules(&modules)?;
        kai_codegen::run_jit(&program).map_err(internal_failure)
    })
}

// Deeply nested input recurses through every phase before a budget trips,
// and debug builds burn thousands of bytes of stack per AST level — the
// default 2 MiB test-thread stack is not enough to reach the diagnostic
// (same reason rustc runs its own passes on a big stack). The helper lives
// with the parser, which needs it first in the pipeline.
use kai_parser::with_big_stack;

fn load_modules(
    entry: &Path,
) -> Result<Vec<kai_driver_modules::LoadedModule>, Failure> {
    crate::modules::load(entry).map_err(|load_failure| Failure {
        phase: "resolve",
        diagnostics: load_failure.diagnostics,
        sources: load_failure.sources,
    })
}

// Re-export alias keeps the import list tidy without a circular feel.
use crate::modules as kai_driver_modules;

fn lower_modules(modules: &[kai_driver_modules::LoadedModule]) -> Result<TypedProgram, Failure> {
    let sources: Vec<(String, String)> = modules
        .iter()
        .map(|m| (m.file.clone(), m.source.clone()))
        .collect();
    let fail = |phase| {
        let sources = sources.clone();
        move |diagnostics: Vec<Diagnostic>| Failure {
            phase,
            diagnostics,
            sources,
        }
    };

    // Global id order == module order (DFS pre-order), so merging is a plain
    // concatenation; per-module tables in Resolution hold indices into it.
    let merged = Program {
        use_decls: Vec::new(),
        fns: modules.iter().flat_map(|m| m.program.fns.clone()).collect(),
        types: modules
            .iter()
            .flat_map(|m| m.program.types.clone())
            .collect(),
    };

    let inputs: Vec<ModuleInput> = modules
        .iter()
        .map(|m| ModuleInput {
            name: &m.name,
            file: &m.file,
            program: &m.program,
        })
        .collect();

    let resolution =
        kai_resolver::analyze_modules(&inputs).map_err(fail("resolve"))?;

    let mut program = kai_typecheck::check_with(&merged, &resolution)
        .map_err(fail("typecheck"))?;
    kai_ownership::resolve(&mut program);
    Ok(program)
}

fn lower(source: &str) -> Result<TypedProgram, Failure> {
    let fail = |phase| move |diagnostics: Vec<Diagnostic>| Failure {
        phase,
        diagnostics,
        sources: Vec::new(),
    };

    let lexed = kai_lexer::lex(source);
    if !lexed.diagnostics.is_empty() {
        return Err(fail("lex")(lexed.diagnostics));
    }

    let ast = kai_parser::parse(&lexed.tokens).map_err(fail("parse"))?;

    // The string API cannot resolve imports (they need a project root, i.e.
    // a file entry point). A predictable user-facing situation reports as a
    // diagnostic, never an internal error (§8).
    if let Some(first) = ast.use_decls.first() {
        return Err(fail("resolve")(vec![Diagnostic::error(
            "modules require a file entry point; run kai build or kai run on an entry file",
            first.span,
        )]));
    }

    let resolution = kai_resolver::analyze(&ast).map_err(fail("resolve"))?;

    let mut program =
        kai_typecheck::check_with(&ast, &resolution).map_err(fail("typecheck"))?;
    kai_ownership::resolve(&mut program);
    Ok(program)
}

fn internal_failure(err: String) -> Failure {
    Failure {
        phase: "codegen",
        diagnostics: vec![internal(err)],
        sources: Vec::new(),
    }
}

fn internal(message: String) -> Diagnostic {
    Diagnostic::error(
        format!("internal codegen error: {message}"),
        kai_diagnostics::Span::new(0, 0),
    )
}
