//! Expression emission.

use crate::context::Ctx;
use inkwell::values::IntValue;
use kai_tast::{TypedExpr, TypedExprKind};

pub(crate) fn emit<'ctx>(ctx: &Ctx<'ctx>, expr: &TypedExpr) -> IntValue<'ctx> {
    match &expr.kind {
        TypedExprKind::IntLit(value) => {
            let ty = ctx.context.i32_type();
            // `true` = signed interpretation of the two's-complement pattern.
            ty.const_int(*value as u64, true)
        }
    }
}
