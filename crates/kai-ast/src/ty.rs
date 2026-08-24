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
    /// `Optional<T>` / `T?` (v0.0.6). ONE semantic form: `T?` is canonical
    /// source sugar that desugars straight to this variant at parse time —
    /// the compiler never carries a second "nullable" concept (§9.9a).
    Optional(Box<Ty>),
    /// `Result<T, E>` (v0.0.6). Deliberately no postfix sugar: a binary type
    /// parameter has no natural unary shorthand (§9.9a).
    Result { ok: Box<Ty>, err: Box<Ty> },
    /// `(params) -> ret` (v0.0.6) — closure/function type. Note the value
    /// syntax keeps its `fn` head (`ClosureLit`); only the TYPE dropped it.
    Closure { params: Vec<Ty>, ret: Box<Ty> },
}

impl Ty {
    pub fn span(&self) -> Span {
        match self {
            Ty::Named(ident) => ident.span,
            Ty::Array(elem) => elem.span(),
            Ty::Optional(inner) => inner.span(),
            Ty::Result { ok, .. } => ok.span(),
            Ty::Closure { params, ret } => params.first().map_or(ret.span(), |p| p.span()),
        }
    }
}
