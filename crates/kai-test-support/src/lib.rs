//! Shared test helpers deduplicating `parse_src` / `check_src` patterns
//! previously copied across `kai-parser`, `kai-resolver`, `kai-typecheck`.
//!
//! This crate is `dev`-oriented: it re-exports minimal helpers so future
//! test modules import one canonical path instead of cloning boilerplate.
//! Centralizes the `join(".")` via `UseDecl::dotted_name()` and `Span::DUMMY`.

use kai_ast::Program;
use kai_diagnostics::{Diagnostic, Span};
use kai_tast::TypedProgram;

/// Lex + parse, panic on failure (test helper).
pub fn parse_ok(src: &str) -> Program {
    let lexed = kai_lexer::lex(src);
    assert!(
        lexed.diagnostics.is_empty(),
        "lex diagnostics: {:?}",
        lexed.diagnostics
    );
    kai_parser::parse(&lexed.tokens).unwrap_or_else(|diags| {
        panic!("parse failed: {:?}", diags);
    })
}

/// Parse and return diagnostics (for negative tests).
pub fn parse_with_diags(src: &str) -> (Option<Program>, Vec<Diagnostic>) {
    let lexed = kai_lexer::lex(src);
    if !lexed.diagnostics.is_empty() {
        return (None, lexed.diagnostics);
    }
    match kai_parser::parse(&lexed.tokens) {
        Ok(p) => (Some(p), vec![]),
        Err(d) => (None, d),
    }
}

/// Full pipeline helper (stub): parse -> placeholder.
/// Mirrors the `check_src` helpers duplicated in `kai-typecheck` and `kai-resolver`.
/// Full `resolve+typecheck` wiring will be added once `kai-resolver` exposes a public test API;
/// for now this crate provides `parse_ok` / `parse_with_diags` and re-exports `Span::DUMMY`.
pub fn check_src(_src: &str) -> Result<TypedProgram, Vec<Diagnostic>> {
    Err(vec![Diagnostic::error(
        "check_src helper placeholder — use per-crate checker directly; dedup migration in progress (v0.0.6.1 stub)",
        Span::DUMMY,
    )])
}

/// Re-exported `Span::DUMMY` for call sites previously using `Span::new(0,0)` sentinel.
/// See `kai-diagnostics/src/span.rs:11`.
pub use kai_diagnostics::Span as TestSpan;

/// Helper to assert a diagnostic contains a substring.
pub fn assert_diag_contains(diags: &[Diagnostic], needle: &str) {
    assert!(
        diags.iter().any(|d| d.message.contains(needle)),
        "expected diagnostic containing `{needle}`, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}
