use crate::context::Ctx;
use inkwell::types::{BasicType, BasicTypeEnum};
use kai_tast::KaiType;

/// Kai value type -> LLVM type. Every new value-carrying `KaiType` variant
/// must be mapped here exactly once.
pub(crate) fn to_llvm<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType) -> BasicTypeEnum<'ctx> {
    match ty {
        KaiType::Int32 => ctx.context.i32_type().into(),
        KaiType::Int64 => ctx.context.i64_type().into(),
        KaiType::Float64 => ctx.context.f64_type().into(),
        KaiType::Bool => ctx.context.bool_type().into(),
        KaiType::Unit => unreachable!("unit has no LLVM value representation"),
        KaiType::Struct(id) => ctx.structs[id.0 as usize].into(),
        // v0.0.5: heap values are POINTERS to `{ rc, len, .. }` headers.
        KaiType::String | KaiType::Array(_) => ctx.context.ptr_type(inkwell::AddressSpace::default()).into(),
        // v0.0.6 (§9.9a): tagged unions are INLINE aggregates —
        // `{ i64 tag, payload }` / `{ i64 tag, ok, err }`. The tag is i64 to
        // avoid padding surprises across heterogeneous payloads.
        KaiType::Optional(inner) => {
            let fields: Vec<BasicTypeEnum> =
                vec![ctx.context.i64_type().into(), to_llvm(ctx, inner)];
            ctx.context.struct_type(&fields, false).into()
        }
        KaiType::Result { ok, err } => {
            // Non-overlapping payloads for v0.0.6 — correctness first; a
            // union layout would need untagged bitcasts for zero gain here.
            let fields: Vec<BasicTypeEnum> = vec![
                ctx.context.i64_type().into(),
                to_llvm(ctx, ok),
                to_llvm(ctx, err),
            ];
            ctx.context.struct_type(&fields, false).into()
        }
        // Closures are fat pointers `{ fn_ptr, env_ptr }` (§9.10); the env
        // header carries the refcount, so the value itself is two words.
        KaiType::Closure { .. } => {
            let ptr = ctx.context.ptr_type(inkwell::AddressSpace::default());
            ctx.context.struct_type(&[ptr.into(), ptr.into()], false).into()
        }
    }
}

/// Function signature type: `void` for unit returns, by-value parameters
/// (§9.3 — callees see copies, so mutation stays local).
pub(crate) fn fn_signature<'ctx>(
    ctx: &Ctx<'ctx>,
    ret: &KaiType,
    params: &[BasicTypeEnum<'ctx>],
) -> inkwell::types::FunctionType<'ctx> {
    let params_meta: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = params
        .iter()
        .map(|ty| inkwell::types::BasicMetadataTypeEnum::from(*ty))
        .collect();
    match ret {
        KaiType::Unit => ctx.context.void_type().fn_type(&params_meta, false),
        other => to_llvm(ctx, other).fn_type(&params_meta, false),
    }
}

/// Zero value of a type, used for dead-path fallback returns. Unit functions
/// return without a value.
pub(crate) fn zero_of<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: &KaiType,
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    match ty {
        KaiType::Unit => None,
        other => Some(match to_llvm(ctx, other) {
            inkwell::types::BasicTypeEnum::IntType(int_ty) => int_ty.const_zero().into(),
            inkwell::types::BasicTypeEnum::FloatType(float_ty) => float_ty.const_zero().into(),
            inkwell::types::BasicTypeEnum::StructType(struct_ty) => struct_ty.const_zero().into(),
            _ => unreachable!("bool is an int type"),
        }),
    }
}
