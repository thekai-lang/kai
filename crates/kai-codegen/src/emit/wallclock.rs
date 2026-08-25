//! Wallclock header emission (§5.1.7): `@wallclock` is unconditionally
//! heap-bearing — construction wraps any built inner value into a fresh
//! `KaiWallclock { rc, instant, dtor, nbytes, payload }` header, and the
//! generated payload destructor cascades into a heap-bearing inner exactly
//! once at rc==0 (the array + ElemDtor precedent, §9.9).

use crate::context::Ctx;
use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, FunctionValue};
use kai_tast::KaiType;

use super::ownership::{emit_release_slot, heap_bearing, type_stem};

// -- v0.0.7 wallclock (§5.1.7) --------------------------------------------------

/// Generated payload destructor for `@wallclock` headers whose inner is
/// heap-bearing: `void @kai.dtor.wall.<inner>(ptr base)` — GEPs the inline
/// payload (index 4) and releases it as `inner`'s storage. Returns None for
/// scalar inners (nothing to cascade; §5.1.7's "int32 @wallclock" case).
pub(crate) fn ensure_wallclock_dtor<'ctx>(
    ctx: &Ctx<'ctx>,
    inner: &KaiType,
) -> Option<FunctionValue<'ctx>> {
    if !heap_bearing(ctx, inner) {
        return None;
    }
    let key = format!("kai.dtor.wall@{}", type_stem(ctx, inner));
    if let Some(existing) = ctx.elem_dtors.borrow().get(&key).copied() {
        return Some(existing);
    }

    let name = sanitize_symbol(&key);
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let function = ctx.module.add_function(&name, llvm, None);
    ctx.elem_dtors.borrow_mut().insert(key, function);

    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let header_ty = crate::types::wallclock_header_ty(ctx, inner);
    let base = function.get_nth_param(0).expect("hdr param").into_pointer_value();
    let payload_ptr = ctx
        .builder
        .build_struct_gep(
            header_ty,
            base,
            crate::types::WALLCLOCK_PAYLOAD_IDX,
            "wall.payload.p",
        )
        .expect("wallclock payload gep");
    emit_release_slot(ctx, inner, payload_ptr);
    let _ = ctx.builder.build_return(None);

    if let Some(saved) = saved_block {
        ctx.builder.position_at_end(saved);
    }
    Some(function)
}

/// Wraps a freshly built INNER value into a heap `@wallclock` header
/// (§5.1.7): allocates via `kai_wallclock_new(now, dtor, nbytes)`, then
/// stores the inner representation into the inline payload slot. Returns
/// the header pointer — the canonical `Temporal Wallclock` runtime shape.
pub(crate) fn wallclock_construct<'ctx>(
    ctx: &Ctx<'ctx>,
    inner: &KaiType,
    inner_value: BasicValueEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    let now_fn = crate::runtime::wallclock_now_fn(ctx);
    let now_val = ctx
        .builder
        .build_call(now_fn, &[], "wall.now")
        .expect("wallclock now call")
        .try_as_basic_value();
    let now = match now_val {
        inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
        _ => unreachable!("kai_wallclock_now returns i64"),
    };

    let inner_llvm = crate::types::to_llvm(ctx, inner);
    let nbytes = inner_llvm
        .size_of()
        .expect("wallclock inner is a sized type");
    let dtor_val: BasicValueEnum<'ctx> =
        match ensure_wallclock_dtor(ctx, inner) {
            Some(f) => f.as_global_value().as_pointer_value().into(),
            None => ctx.context.ptr_type(Default::default()).const_zero().into(),
        };
    let new_fn = crate::runtime::wallclock_new_fn(ctx);
    let hdr_val = ctx
        .builder
        .build_call(
            new_fn,
            &[
                now.into(),
                dtor_val.into(),
                nbytes.into(),
            ],
            "wall.hdr",
        )
        .expect("wallclock new call")
        .try_as_basic_value();
    let hdr = match hdr_val {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        _ => unreachable!("kai_wallclock_new returns ptr"),
    };

    let header_ty = crate::types::wallclock_header_ty(ctx, inner);
    let payload_ptr = ctx
        .builder
        .build_struct_gep(
            header_ty,
            hdr,
            crate::types::WALLCLOCK_PAYLOAD_IDX,
            "wall.payload.p",
        )
        .expect("wallclock payload gep");
    let _ = ctx.builder.build_store(payload_ptr, inner_value);
    hdr.into()
}

/// LLVM symbol-safe version of a helper stem: letters, digits, `.` and `_`
/// survive; everything else becomes `_`.
fn sanitize_symbol(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
