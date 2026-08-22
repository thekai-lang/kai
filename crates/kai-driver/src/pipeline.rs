//! Pipeline orchestration: lex -> parse -> resolve -> typecheck -> codegen.
//! Each phase runs only after the previous one produced no diagnostics; the
//! first failing phase's diagnostics are reported and compilation stops.

use kai_diagnostics::Diagnostic;
use kai_tast::TypedProgram;

#[derive(Debug, Clone)]
pub struct Failure {
    pub phase: &'static str,
    pub diagnostics: Vec<Diagnostic>,
}

/// Full compile to textual LLVM IR (module verified).
pub fn compile(source: &str) -> Result<String, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::compile_ir("kai_module", &program).map_err(|err| Failure {
            phase: "codegen",
            diagnostics: vec![internal(err)],
        })
    })
}

/// JIT-compile and execute `main`, returning its `int32` result.
pub fn jit(source: &str) -> Result<i32, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::run_jit(&program).map_err(|err| Failure {
            phase: "codegen",
            diagnostics: vec![internal(err)],
        })
    })
}

/// Runs the pipeline on a dedicated large-stack thread. Deeply nested input
/// recurses through the parser/typechecker before the depth budget trips, and
/// debug builds burn thousands of bytes of stack per AST level — the default
/// 2 MiB test-thread stack is not enough to reach the diagnostic (same reason
/// rustc runs its own passes on a big stack).
fn with_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    const STACK: usize = 64 * 1024 * 1024;
    match std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn compiler thread")
        .join()
    {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn lower(source: &str) -> Result<TypedProgram, Failure> {
    let fail = |phase| move |diagnostics| Failure { phase, diagnostics };

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

    kai_typecheck::check_with(&ast, &resolution).map_err(fail("typecheck"))
}

fn internal(message: String) -> Diagnostic {
    Diagnostic::error(
        format!("internal codegen error: {message}"),
        kai_diagnostics::Span::new(0, 0),
    )
}
