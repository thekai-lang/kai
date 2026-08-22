use kai_diagnostics::Span;

/// A source-bound name. Shape only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}
