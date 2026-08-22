use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct IntLit {
    pub value: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    IntLit(IntLit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
