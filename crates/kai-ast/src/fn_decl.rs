use crate::ident::Ident;
use crate::param::Param;
use crate::stmt::Block;
use crate::ty::Ty;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// `public fn` — visible through an importing module's alias; a plain
    /// `fn` is module-private (§3.6).
    pub is_public: bool,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Block,
    pub span: Span,
}
