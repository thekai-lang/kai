//! Diagnostic model: {message, span, severity}. Facade only.

mod diagnostic;
mod severity;
mod span;

pub use diagnostic::Diagnostic;
pub use severity::Severity;
pub use span::{LineCol, SourceMap, Span};
