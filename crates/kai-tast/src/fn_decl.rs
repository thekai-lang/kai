use crate::stmt::{TypedBlock, TypedStmt};
use crate::symbol::FunctionId;
use crate::ty::KaiType;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFnDecl {
    pub id: FunctionId,
    /// Kept as the LLVM symbol name; all internal references use `id`.
    pub name: String,
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
