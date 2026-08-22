use kai_diagnostics::Diagnostic;
use kai_lexer::Token;

pub fn expected(expected: impl Into<String>, found: &Token) -> Diagnostic {
    Diagnostic::error(
        format!("expected {}, found {}", expected.into(), found.describe()),
        found.span,
    )
}

pub fn custom(message: impl Into<String>, span: kai_diagnostics::Span) -> Diagnostic {
    Diagnostic::error(message, span)
}
