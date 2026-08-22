use crate::severity::Severity;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Error,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_error_diagnostic() {
        let d = Diagnostic::error("unexpected token", Span::new(0, 3));
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.span.end, 3);
    }
}
