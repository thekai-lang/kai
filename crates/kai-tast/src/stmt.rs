use crate::expr::{BinaryOp, TypedExpr};
use crate::symbol::LocalId;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmt {
    Return(Option<TypedExpr>),
    Let(TypedLet),
    Assign(TypedAssign),
    If(TypedIf),
    /// Bare nested block; exists purely for scoping semantics.
    Block(TypedBlock),
    Expr(TypedExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedLet {
    pub local: LocalId,
    /// Kept only for LLVM value names / future diagnostics; uniqueness of
    /// `local` is what codegen relies on.
    pub name: String,
    pub init: TypedExpr,
}

/// `op == None` is plain store; `Some(op)` is read-modify-write
/// (`x += e` lowers to load, add, store).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedAssign {
    pub local: LocalId,
    pub op: Option<BinaryOp>,
    pub value: TypedExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedIf {
    pub cond: TypedExpr,
    pub then_block: TypedBlock,
    /// Absent = no else branch (fall-through).
    pub else_block: Option<TypedBlock>,
}
