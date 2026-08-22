//! Untyped AST definitions. Shape only, no logic — parser output.

pub mod expr;
pub mod fn_decl;
pub mod ident;
pub mod param;
pub mod program;
pub mod stmt;
pub mod ty;

pub use expr::{Expr, ExprKind, IntLit};
pub use fn_decl::FnDecl;
pub use ident::Ident;
pub use param::Param;
pub use program::Program;
pub use stmt::{Block, Stmt, StmtKind};
pub use ty::Ty;
