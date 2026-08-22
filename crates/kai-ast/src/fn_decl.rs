use crate::ident::Ident;
use crate::param::Param;
use crate::stmt::Block;
use crate::ty::Ty;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Block,
    pub span: Span,
}
