use crate::expr::TypedExpr;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmt {
    Return(Option<TypedExpr>),
}
