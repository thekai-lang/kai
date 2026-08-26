#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Tagged-union emission — `None`, `??`, `unwrap_or`, `catch` helpers.
use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::values::BasicValueEnum;
use kai_tast::{KaiType, TypedExpr};

use super::emit;

pub(crate) fn i64_const<'ctx>(ctx: &Ctx<'ctx>, v: u64) -> inkwell::values::IntValue<'ctx> {
    ctx.context.i64_type().const_int(v, false)
}

/// `{ tag = 1 }` with a zeroed payload — the None/absent shape. Payload
/// fields are zeroed per LLVM kind; nested aggregates stay undef (they are
/// never read while the tag says absent).

pub(crate) fn tagged_none_const<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType) -> BasicValueEnum<'ctx> {
    let llvm = crate::types::to_llvm(ctx, ty).into_struct_type();
    let mut fields: Vec<BasicValueEnum<'ctx>> = vec![i64_const(ctx, 1).into()];
    for idx in 1..llvm.count_fields() {
        fields.push(zero_of(ctx, llvm.get_field_type_at_index(idx).expect("field")));
    }
    llvm.const_named_struct(&fields).into()
}

pub(crate) fn zero_of<'ctx>(ctx: &Ctx<'ctx>, ty: inkwell::types::BasicTypeEnum<'ctx>) -> BasicValueEnum<'ctx> {
    match ty {
        inkwell::types::BasicTypeEnum::PointerType(_) => {
            ctx.context.ptr_type(Default::default()).const_zero().into()
        }
        inkwell::types::BasicTypeEnum::IntType(t) => t.const_zero().into(),
        inkwell::types::BasicTypeEnum::FloatType(t) => t.const_zero().into(),
        other => other.into_struct_type().get_undef().into(),
    }
}

/// Shared `??` / `.unwrap_or` shape: both are "active branch's payload or
/// the fallback". Tag 0 = Some/Ok in either layout; payload sits at member 1.

pub(crate) fn lazy_select<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    receiver: &TypedExpr,
    fallback: &TypedExpr,
    result_ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let recv = emit(ctx, frame, receiver).into_struct_value();
    let tag = ctx
        .builder
        .build_extract_value(recv, 0, "tag")
        .expect("tag")
        .into_int_value();
    let active = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, tag, i64_const(ctx, 0), "active")
        .expect("tag cmp");

    let result_llvm = crate::types::to_llvm(ctx, result_ty);
    let slot = crate::emit::alloca_in_entry(
        ctx,
        crate::emit::current_function(ctx),
        result_llvm,
        "co.r",
    );
    let some_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.some");
    let else_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.fallback");
    let join_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.join");
    let _ = ctx
        .builder
        .build_conditional_branch(active, some_bb, else_bb);

    // Active: forward the payload bits untouched — the consumer's retain
    // (borrow semantics) balances against its later release.
    ctx.builder.position_at_end(some_bb);
    let payload = ctx
        .builder
        .build_extract_value(recv, 1, "payload")
        .expect("payload");
    let _ = ctx.builder.build_store(slot, payload);
    let _ = ctx.builder.build_unconditional_branch(join_bb);

    // Fallback: evaluate lazily; store directly to the result slot.
    // NO release here — both branches produce correctly-owned values for
    // the consumer (some_bb borrows from recv, else_bb owns fresh temp).
    // The old code released the creator's reference here, but that freed
    // heap fields BEFORE co.join could read them (v0.0.8.1 BUG).
    ctx.builder.position_at_end(else_bb);
    let d = emit(ctx, frame, fallback);
    let _ = ctx.builder.build_store(slot, d);
    let _ = ctx.builder.build_unconditional_branch(join_bb);

    ctx.builder.position_at_end(join_bb);
    ctx.builder
        .build_load(result_llvm, slot, "co.v")
        .expect("load coalesced")
}


/// True when the current insert block already has a terminator (an early
/// `return` inside a catch body leaves it that way).

pub(crate) fn terminated_here(ctx: &Ctx<'_>) -> bool {
    ctx.builder
        .get_insert_block()
        .and_then(|b| b.get_terminator())
        .is_some()
}

