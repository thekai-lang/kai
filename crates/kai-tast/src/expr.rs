use super::ty::KaiType;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: KaiType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    /// Value is already range-checked to fit `i32` by the type checker.
    IntLit(i32),
}

impl TypedExpr {
    pub fn new(kind: TypedExprKind, ty: KaiType) -> Self {
        Self { kind, ty }
    }
}
