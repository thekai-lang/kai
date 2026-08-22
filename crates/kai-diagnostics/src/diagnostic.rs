use crate::severity::Severity;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub severity: Severity,
    /// Source file the span indexes into (§8 constraint 6): `None` in
    /// single-file phases (v0.0.1–v0.0.3); populated from v0.0.4 once one
    /// compilation spans several files. Holds the path relative to the
    /// project root as displayed to users.
    pub file: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Error,
            file: None,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Warning,
            file: None,
        }
    }

    /// Attaches the source file this diagnostic's span belongs to.
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
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
