use kai_diagnostics::Span;

/// Binary operators. Precedence lives in the parser; this enum only carries
/// which operator was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinaryOp {
    pub fn describe(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntLit {
    pub value: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatLit {
    pub value: f64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub op_span: Span,
    pub operand: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub op_span: Span,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

/// `callee(args)` — v0.0.3 restricts valid callees to top-level functions.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
}

/// `base.field` — struct field read (v0.0.3); chains nest naturally
/// (`line.start.x` is FieldAccess(FieldAccess(Ident, start), x)).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccessExpr {
    pub base: Box<Expr>,
    pub field: Ident,
}

/// `Name { field: expr, ... }` — struct literal (v0.0.3).
#[derive(Debug, Clone, PartialEq)]
pub struct StructLitExpr {
    pub name: Ident,
    pub fields: Vec<FieldInit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    IntLit(IntLit),
    FloatLit(FloatLit),
    BoolLit {
        value: bool,
        span: Span,
    },
    Ident(Ident),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
    FieldAccess(FieldAccessExpr),
    StructLit(StructLitExpr),
    /// Poisoned node produced only by parser error recovery (e.g. an
    /// expression nested past the recursion budget). Downstream phases treat
    /// it as an error marker, never as compilable code.
    Invalid,
}

use crate::ident::Ident;

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
