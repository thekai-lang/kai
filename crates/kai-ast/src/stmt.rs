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

/// `for name in expr { ... }` — iterates an array, BORROWING each element
/// per iteration (§9.9): the loop variable never owns, the array stays
/// owner throughout and after the loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub binding: Ident,
    pub iterable: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `return;` carries `None` (unit return, usable from v0.0.2).
    Return(Option<Expr>),
    Let(LetStmt),
    Assign(AssignStmt),
    If(IfStmt),
    For(ForStmt),
    /// Bare nested block: its own variable scope (§9.3 shadowing rules).
    Block(Block),
    /// `_ = expr;` (v0.0.6, §9.9b) — the sole explicit-discard form. The
    /// expression evaluates normally under ordinary ownership rules; its
    /// value simply isn't bound. This is also the legal way to discard an
    /// `Optional`/`Result` without triggering the §9.9a diagnostic.
    Discard(Expr),
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
