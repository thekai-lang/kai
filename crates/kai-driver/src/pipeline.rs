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
    let program = lower(source)?;
    kai_codegen::compile_ir("kai_module", &program).map_err(|err| Failure {
        phase: "codegen",
        diagnostics: vec![internal(err)],
    })
}

/// JIT-compile and execute `main`, returning its `int32` result.
pub fn jit(source: &str) -> Result<i32, Failure> {
    let program = lower(source)?;
    kai_codegen::run_jit(&program).map_err(|err| Failure {
        phase: "codegen",
        diagnostics: vec![internal(err)],
    })
}

fn lower(source: &str) -> Result<TypedProgram, Failure> {
    let fail = |phase| move |diagnostics| Failure { phase, diagnostics };

    let lexed = kai_lexer::lex(source);
    if !lexed.diagnostics.is_empty() {
        return Err(fail("lex")(lexed.diagnostics));
    }

    let ast = kai_parser::parse(&lexed.tokens).map_err(fail("parse"))?;

    let resolve_diags = kai_resolver::check_entry(&ast);
    if !resolve_diags.is_empty() {
        return Err(fail("resolve")(resolve_diags));
    }

    kai_typecheck::check(&ast).map_err(fail("typecheck"))
}

fn internal(message: String) -> Diagnostic {
    Diagnostic::error(
        format!("internal codegen error: {message}"),
        kai_diagnostics::Span::new(0, 0),
    )
}
