use crate::ident::Ident;
use crate::ty::Ty;

/// Function parameter shape. Not constructible until v0.0.3 (parameters enter
/// the language then); defined now so AST shape is stable from commit #1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Ty,
}
