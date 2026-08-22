use crate::context::Ctx;
use inkwell::types::{BasicType, BasicTypeEnum};
use kai_tast::KaiType;

/// Kai value type -> LLVM scalar type. Every new value-carrying `KaiType`
/// variant must be mapped here exactly once.
pub(crate) fn to_llvm<'ctx>(ctx: &Ctx<'ctx>, ty: KaiType) -> BasicTypeEnum<'ctx> {
    match ty {
        KaiType::Int32 => ctx.context.i32_type().into(),
        KaiType::Int64 => ctx.context.i64_type().into(),
        KaiType::Float64 => ctx.context.f64_type().into(),
        KaiType::Bool => ctx.context.bool_type().into(),
        KaiType::Unit => unreachable!("unit has no LLVM value representation"),
    }
}

/// Function signature type for a given Kai return type (`void` for unit).
pub(crate) fn fn_signature<'ctx>(
    ctx: &Ctx<'ctx>,
    ret: KaiType,
) -> inkwell::types::FunctionType<'ctx> {
    match ret {
        KaiType::Unit => ctx.context.void_type().fn_type(&[], false),
        other => to_llvm(ctx, other).fn_type(&[], false),
    }
}

/// Zero value of a type, used for dead-path fallback returns. Unit functions
/// return without a value.
pub(crate) fn zero_of<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: KaiType,
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    match ty {
        KaiType::Unit => None,
        other => Some(match to_llvm(ctx, other) {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => int_ty.const_zero().into(),
            inkwell::types::BasicTypeEnum::FloatType(float_ty) => float_ty.const_zero().into(),
            _ => unreachable!("scalar types only"),
        }),
    }
}
