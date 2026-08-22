use crate::assign::{AssignOp, AssignTarget};
use crate::expr::Expr;
use crate::ident::Ident;
use crate::ty::Ty;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub name: Ident,
    pub ty: Option<Ty>,
    pub init: Expr,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub target: AssignTarget,
    pub op: AssignOp,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    /// `else if` chains nest here as a single-statement block.
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `return;` carries `None` (unit return, usable from v0.0.2).
    Return(Option<Expr>),
    Let(LetStmt),
    Assign(AssignStmt),
    If(IfStmt),
    /// Bare nested block: its own variable scope (§9.3 shadowing rules).
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}
