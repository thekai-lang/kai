use crate::context::Ctx;
use inkwell::types::IntType;
use kai_tast::KaiType;

/// Kai type -> LLVM scalar type. Every new `KaiType` variant must be mapped
/// here exactly once.
pub(crate) fn to_llvm<'ctx>(ctx: &Ctx<'ctx>, ty: KaiType) -> IntType<'ctx> {
    match ty {
        KaiType::Int32 => ctx.context.i32_type(),
    }
}
