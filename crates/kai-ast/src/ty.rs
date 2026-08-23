use crate::ident::Ident;
use kai_diagnostics::Span;

/// Syntactic type reference. Primitives are plain names here (`int32`);
/// resolution to concrete types happens in the type checker, never the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Named(Ident),
    /// `T[]` (v0.0.5). Arrays are unconditionally heap-bearing (§9.1),
    /// whatever the element type.
    Array(Box<Ty>),
}

impl Ty {
    pub fn span(&self) -> Span {
        match self {
            Ty::Named(ident) => ident.span,
            Ty::Array(elem) => elem.span(),
        }
    }
}
