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

pub use assign::{AssignOp, AssignTarget, PlaceStep};
pub use expr::{
    ArrayLitExpr, BinaryExpr, BinaryOp, CallExpr, CatchExpr, ClosureLitExpr, CoalesceExpr, ErrLitExpr, Expr,
    ExprKind, FieldAccessExpr, FieldInit, FloatLit, IndexExpr, IntLit, OkLitExpr, SomeLitExpr, StrLitExpr,
    StructLitExpr, UnaryExpr, UnaryOp,
};
pub use fn_decl::{EffectName, EffectSet, FnDecl};
pub use ident::Ident;
pub use param::Param;
pub use program::Program;
pub use stmt::{AssignStmt, Block, ForStmt, IfStmt, LetStmt, Stmt, StmtKind, WhileStmt};
pub use ty::{DurationLit, DurationUnit, TemporalOrigin, Ty};
pub use type_decl::{FieldDecl, TypeDecl};
pub use use_decl::UseDecl;
