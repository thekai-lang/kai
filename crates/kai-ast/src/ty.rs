use crate::ident::Ident;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationUnit {
    Ms,
    S,
    M,
    H,
    D,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationLit {
    pub value: u64,
    pub unit: DurationUnit,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalOrigin {
    Local,
    Wallclock,
}

/// Syntactic type reference. Primitives are plain names here (`int32`);
/// resolution to concrete types happens in the type checker, never the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Path(Vec<Ident>),
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
    /// `T @local(d)` / `T @wallclock(d)` (v0.0.7, §5.1). Postfix temporal
    /// modifier, same position as `T?` and `T[]` (§9).
    Temporal {
        inner: Box<Ty>,
        origin: TemporalOrigin,
        duration: DurationLit,
    },
}

impl Ty {
    pub fn span(&self) -> Span {
        match self {
            Ty::Path(path) => {
                let start = path.first().unwrap().span;
                let end = path.last().unwrap().span;
                kai_diagnostics::Span::new(start.start, end.end)
            },
            Ty::Array(elem) => elem.span(),
            Ty::Optional(inner) => inner.span(),
            Ty::Result { ok, .. } => ok.span(),
            Ty::Closure { params, ret } => params.first().map_or(ret.span(), |p| p.span()),
            Ty::Temporal { inner, .. } => inner.span(),
        }
    }
}
