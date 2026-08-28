//! Hand-written recursive-descent parser: tokens -> untyped AST.
//! Errors are diagnostics with spans; parsing never panics on bad input.

pub(crate) mod sql;
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
    // Deeply nested input recurses until the expression budget trips, and
    // debug builds burn kilobytes of native stack per AST level. The budget
    // bounds *work*, not stack depth, so parsing always runs on a dedicated
    // large-stack thread instead of trusting the caller's (rustc does the
    // same for its own passes).
    let owned = tokens.to_vec();
    with_big_stack(move || {
        let mut parser = Parser::new(&owned);
        let program = decl::program(&mut parser);

        if parser.diagnostics.is_empty() {
            Ok(program)
        } else {
            Err(parser.diagnostics)
        }
    })
}

/// Runs `f` on a 64 MiB-stack thread, re-raising any panic unchanged.
/// Shared with the driver pipeline: every phase recurses over user-shaped
/// trees before a budget trips, so all of them need the same headroom.
pub fn with_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    const STACK: usize = 64 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn parser thread");
    match handle.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod v0003_tests;
#[cfg(test)]
mod v0005_surface_tests;
#[cfg(test)]
mod v0006_tests;
