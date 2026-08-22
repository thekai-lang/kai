//! Statement emission.

use crate::context::Ctx;
use crate::emit::expr;
use kai_tast::TypedStmt;

pub(crate) fn emit(ctx: &Ctx, stmt: &TypedStmt) {
    match stmt {
        TypedStmt::Return(value) => ret(ctx, value.as_ref()),
    }
}

fn ret(ctx: &Ctx, value: Option<&kai_tast::TypedExpr>) {
    match value {
        Some(expr_ast) => {
            let value = expr::emit(ctx, expr_ast);
            let _ = ctx.builder.build_return(Some(&value));
        }
        None => {
            let _ = ctx.builder.build_return(None);
        }
    }
}
