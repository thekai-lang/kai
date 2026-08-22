//! Untyped AST definitions. Shape only, no logic — parser output.

pub mod assign;
pub mod expr;
pub mod fn_decl;
pub mod ident;
pub mod param;
pub mod program;
pub mod stmt;
pub mod ty;
pub mod type_decl;
pub mod use_decl;

pub use assign::{AssignOp, AssignTarget};
pub use expr::{
    BinaryExpr, BinaryOp, CallExpr, Expr, ExprKind, FieldAccessExpr, FieldInit, FloatLit, IntLit,
    StructLitExpr, UnaryExpr, UnaryOp,
};
pub use fn_decl::FnDecl;
pub use ident::Ident;
pub use param::Param;
pub use program::Program;
pub use stmt::{AssignStmt, Block, IfStmt, LetStmt, Stmt, StmtKind};
pub use ty::Ty;
pub use type_decl::{FieldDecl, TypeDecl};
pub use use_decl::UseDecl;
