use crate::symbol::LocalId;
use crate::ty::KaiType;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: KaiType,
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    /// Value fits the width named by `ty` (range-checked by the type checker).
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    LocalRef(LocalId),
    Neg(Box<TypedExpr>),
    Not(Box<TypedExpr>),
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
    },
}

impl TypedExpr {
    pub fn new(kind: TypedExprKind, ty: KaiType) -> Self {
        Self { kind, ty }
    }

    /// Integer literal constructor that picks the right width.
    pub fn int_lit(value: i64, ty: KaiType) -> Self {
        Self::new(TypedExprKind::IntLit(value), ty)
    }
}
