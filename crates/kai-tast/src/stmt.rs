use crate::expr::{BinaryOp, TypedExpr};
use crate::symbol::{LocalId, StructId};

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
    For(crate::stmt::TypedFor),
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

/// One hop of a field-place path (`root.a.b` carries two steps), resolved to
/// the declaring struct and the field's position in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldStep {
    pub struct_id: StructId,
    pub field: u16,
}

/// One hop of the generalized v0.0.5 Place: a field projection or an array
/// index projection. The index EXPRESSION rides along — it evaluates fresh
/// at every assignment site (§9.3).
#[derive(Debug, Clone, PartialEq)]
pub enum TypedPlaceStep {
    Field(FieldStep),
    Index(Box<TypedExpr>),
}

/// `op == None` is plain store; `Some(op)` is read-modify-write
/// (`x += e` lowers to load, add, store). The write goes to `root`, or —
/// when `path` is non-empty — through the field chain starting at `root`.
/// Mutability was enforced upstream against the ROOT binding (§9.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedAssign {
    pub root: LocalId,
    pub path: Vec<TypedPlaceStep>,
    pub op: Option<BinaryOp>,
    pub value: TypedExpr,
}

/// `for name in array { body }`. `binding_local` is declared ONCE per loop
/// (immutable, element-typed) and re-stored each iteration — the loop
/// variable borrows, it never owns (§9.9).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedFor {
    pub binding_local: LocalId,
    pub binding_name: String,
    pub iterable: TypedExpr,
    pub body: TypedBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedIf {
    pub cond: TypedExpr,
    pub then_block: TypedBlock,
    /// Absent = no else branch (fall-through).
    pub else_block: Option<TypedBlock>,
}
