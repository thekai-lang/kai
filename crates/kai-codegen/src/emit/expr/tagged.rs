#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Tagged-union emission — `None`, `??`, `unwrap_or`, `catch` helpers.
use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::values::BasicValueEnum;
use kai_tast::{KaiType, TypedExpr, TypedExprKind};

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

    // Active: extract payload, store to result slot, then RETAIN if
    // heap-bearing — the consumer owns this copy (§9.5 boundary rule).
    // Without this retain, scope-exit release would decrement below the
    // original owner's claim (v0.0.8.2 BUG-5 fix).
    ctx.builder.position_at_end(some_bb);
    let payload = ctx
        .builder
        .build_extract_value(recv, 1, "payload")
        .expect("payload");
    let _ = ctx.builder.build_store(slot, payload);
    if crate::emit::ownership::heap_bearing(ctx, result_ty) {
        match result_ty {
            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                let hdr = ctx.builder.build_load(
                    crate::types::to_llvm(ctx, result_ty),
                    slot,
                    "some.ret.hdr",
                ).expect("payload header load");
                crate::emit::ownership::retain_header(ctx, hdr);
            }
            KaiType::Struct(_) => {
                crate::emit::ownership::retain_struct_copy(ctx, result_ty, slot);
            }
            // Temporal (§5.1.7): @wallclock always heap-bearing (header);
            // @local delegates to inner retain (zero-cost repr).
            KaiType::Temporal { inner, origin, .. } => {
                let loaded = ctx.builder.build_load(
                    crate::types::to_llvm(ctx, &KaiType::Temporal {
                        inner: inner.clone(),
                        origin: origin.clone(),
                        duration: kai_tast::DurationLit { value: 0, unit: kai_tast::DurationUnit::S },
                    }),
                    slot,
                    "some.ret.temporal",
                ).expect("payload load");
                match origin {
                    kai_tast::TemporalOrigin::Wallclock => {
                        // Retain the wallclock header itself; cascades via dtor.
                        crate::emit::ownership::retain_header(ctx, loaded);
                    }
                    _ => {
                        // @local: delegate to inner retain (same repr).
                        match inner.as_ref() {
                            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                                crate::emit::ownership::retain_header(ctx, loaded);
                            }
                            KaiType::Struct(_) => {
                                let tmp = crate::emit::alloca_in_entry(
                                    ctx,
                                    crate::emit::current_function(ctx),
                                    crate::types::to_llvm(ctx, inner),
                                    "some.retain.tmp",
                                );
                                let _ = ctx.builder.build_store(tmp, loaded);
                                crate::emit::ownership::retain_struct_copy(ctx, inner, tmp);
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let _ = ctx.builder.build_unconditional_branch(join_bb);

    // Fallback: evaluate lazily; store directly to the result slot.
    //
    // §9.5/v0.22 branch-level claim normalization: after storing to co.r,
    // normalize claims so exactly ONE claim survives past join:
    // - fresh fallback (owned temp): retain for consumer, release creation
    // - borrowed fallback: retain only (owner keeps their claim)
    // Without this normalization: fresh leaked at rc=1 (pre-da88704 was
    // corruption from premature release). Neither is acceptable.
    ctx.builder.position_at_end(else_bb);
    let d = emit(ctx, frame, fallback);
    let _ = ctx.builder.build_store(slot, d);

    let fallback_is_fresh = matches!(
        &fallback.kind,
        TypedExprKind::IntLit(_)
            | TypedExprKind::StrLit { .. }
            | TypedExprKind::ArrayLit { .. }
            | TypedExprKind::StructLit { .. }
            | TypedExprKind::Call { .. }
            | TypedExprKind::SomeLit(_)
            | TypedExprKind::OkLit(_)
            | TypedExprKind::ErrLit(_)
            | TypedExprKind::ClosureLit(_)
            | TypedExprKind::UnwrapOr { .. }
            | TypedExprKind::Coalesce { .. }
    );

    if crate::emit::ownership::heap_bearing(ctx, result_ty) {
        // Step 1: retain for the consumer (both fresh and borrow)
        match result_ty {
            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                crate::emit::ownership::retain_header(ctx, d);
            }
            KaiType::Struct(_) => {
                let tmp = crate::emit::alloca_in_entry(
                    ctx,
                    crate::emit::current_function(ctx),
                    result_llvm,
                    "co.retain.tmp",
                );
                let _ = ctx.builder.build_store(tmp, d);
                crate::emit::ownership::retain_struct_copy(ctx, result_ty, tmp);
            }
            KaiType::Optional(_) | KaiType::Result { .. } => {
                let tmp = crate::emit::alloca_in_entry(
                    ctx,
                    crate::emit::current_function(ctx),
                    result_llvm,
                    "co.retain.tmp",
                );
                let _ = ctx.builder.build_store(tmp, d);
                crate::emit::ownership_tagged::retain_tagged_copy(ctx, result_ty, tmp);
            }
            KaiType::Temporal { inner, origin, .. } => {
                match origin {
                    kai_tast::TemporalOrigin::Wallclock => {
                        // Retain the wallclock header itself; cascades via dtor.
                        crate::emit::ownership::retain_header(ctx, d);
                    }
                    _ => {
                        // @local: delegate to inner retain (same repr).
                        match inner.as_ref() {
                            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                                crate::emit::ownership::retain_header(ctx, d);
                            }
                            KaiType::Struct(_) => {
                                let tmp = crate::emit::alloca_in_entry(
                                    ctx,
                                    crate::emit::current_function(ctx),
                                    crate::types::to_llvm(ctx, inner),
                                    "co.retain.tmp",
                                );
                                let _ = ctx.builder.build_store(tmp, d);
                                crate::emit::ownership::retain_struct_copy(ctx, inner, tmp);
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        // Step 2: release creation claim ONLY for fresh owned temps.
        // For borrowed fallbacks, the original owner keeps their claim —
        // releasing here would be premature/double-free.
        if fallback_is_fresh {
            match result_ty {
                KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                    crate::emit::ownership::release_header_value(ctx, d);
                }
                KaiType::Struct(_) | KaiType::Optional(_) | KaiType::Result { .. } => {
                    let tmp = crate::emit::alloca_in_entry(
                        ctx,
                        crate::emit::current_function(ctx),
                        result_llvm,
                        "co.release.tmp",
                    );
                    let _ = ctx.builder.build_store(tmp, d);
                    crate::emit::ownership::emit_release_slot(ctx, result_ty, tmp);
                }
                KaiType::Temporal { origin, .. } => {
                    match origin {
                        kai_tast::TemporalOrigin::Wallclock => {
                            // Release creation claim for the wallclock header.
                            crate::emit::ownership::release_header_value(ctx, d);
                        }
                        _ => {
                            // @local: delegate to inner release via slot.
                            let tmp = crate::emit::alloca_in_entry(
                                ctx,
                                crate::emit::current_function(ctx),
                                result_llvm,
                                "co.release.tmp",
                            );
                            let _ = ctx.builder.build_store(tmp, d);
                            crate::emit::ownership::emit_release_slot(ctx, result_ty, tmp);
                        }
                    }
                }
                _ => {}
            }
        }
    }

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

