use crate::ident::Ident;
use crate::ty::Ty;
use kai_diagnostics::Span;

/// `type Name = { field: Type; ... }` — struct declaration (v0.0.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    /// `public type` — same visibility rule as `public fn` (§3.6).
    pub is_public: bool,
    pub name: Ident,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: Ident,
    pub ty: Ty,
}
