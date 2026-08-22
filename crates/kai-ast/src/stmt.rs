use crate::expr::Expr;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `return;` carries `None` (unit return, usable from v0.0.2).
    Return(Option<Expr>),
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
