//! Typed AST: type-checker output, codegen input.
//! Shape only, no logic — and strictly separate from `kai-ast` (§8.1).

pub mod expr;
pub mod fn_decl;
pub mod program;
pub mod stmt;
pub mod symbol;
pub mod ty;

pub use expr::{BinaryOp, TypedExpr, TypedExprKind};
pub use fn_decl::{TypedFnDecl, TypedParam};
pub use program::TypedProgram;
pub use stmt::{FieldStep, TypedAssign, TypedBlock, TypedIf, TypedLet, TypedStmt};
pub use symbol::{FunctionId, LocalId, StructId};
pub use ty::KaiType;
