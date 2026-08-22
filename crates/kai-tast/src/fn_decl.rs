use crate::stmt::{TypedBlock, TypedStmt};
use crate::symbol::FunctionId;
use crate::symbol::LocalId;
use crate::ty::KaiType;

/// A parameter, already bound to its local slot by the type checker. `mut`
/// is compile-time only (§9.3): it never changes the ABI.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub local: LocalId,
    /// Kept as the LLVM argument name.
    pub name: String,
    pub ty: KaiType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFnDecl {
    pub id: FunctionId,
    /// Kept as the LLVM symbol name; all internal references use `id`.
    pub name: String,
    pub params: Vec<TypedParam>,
    pub ret: KaiType,
    pub body: TypedBlock,
}

impl TypedFnDecl {
    pub fn has_return(&self) -> bool {
        self.body
            .stmts
            .iter()
            .any(|stmt| matches!(stmt, TypedStmt::Return(_)))
    }
}
