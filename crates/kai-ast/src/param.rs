use crate::ident::Ident;
use crate::ty::Ty;

/// Function parameter. `mutable` mirrors the `mut` annotation (§9.3): it
/// gates mutation of the parameter inside the callee — for stack types a
/// purely local, compile-time permission (zero ABI difference).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Ty,
    pub mutable: bool,
}
