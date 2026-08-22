use crate::ident::Ident;

/// Syntactic type reference. Primitives are plain names here (`int32`);
/// resolution to concrete types happens in the type checker, never the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Named(Ident),
}

impl Ty {
    pub fn span(&self) -> kai_diagnostics::Span {
        match self {
            Ty::Named(ident) => ident.span,
        }
    }
}
