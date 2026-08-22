//! Diagnostic model: {message, span, severity}.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span, severity: Severity::Error }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_error_diagnostic() {
        let d = Diagnostic::error("unexpected token", Span { start: 0, end: 3 });
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.span.end, 3);
    }
}
