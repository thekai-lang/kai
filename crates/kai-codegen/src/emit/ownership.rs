//! Ownership event emission (§9): the mechanical half. The ownership pass
//! already decided WHERE retain/release happen; this module decides WHAT
//! they do for a given type:
//!
//! - `string` / `T[]` — one refcount op on the shared uniform header
//!   (`kai_retain` / `kai_release`). Array element destructors ride inside
//!   the header (`kai_array_new`'s `dtor` argument) so they run exactly
//!   once, when the last owner releases.
//! - heap-bearing structs — generated per-struct helpers that retain or
//!   release each heap-bearing FIELD individually (§9.5: per-field, never
//!   whole-struct refcounting), recursively through nested structs.

use crate::context::Ctx;
use crate::emit::expr;
use inkwell::values::{BasicValueEnum, FunctionValue};
use kai_tast::KaiType;

/// Does a value of `ty` own heap memory (directly or through fields)?
pub(crate) fn heap_bearing(ctx: &Ctx<'_>, ty: &KaiType) -> bool {
    match ty {
        KaiType::String | KaiType::Array(_) => true,
        KaiType::Struct(id) => ctx.struct_fields[id.0 as usize]
            .iter()
            .any(|f| heap_bearing(ctx, f)),
        // Tagged unions (§9.9a): conditional on the active payload's shape —
        // Result counts either branch. Closures: unconditionally heap-bearing
        // (v0.13), the environment header always exists.
        KaiType::Optional(inner) => heap_bearing(ctx, inner),
        KaiType::Result { ok, err } => heap_bearing(ctx, ok) || heap_bearing(ctx, err),
        // Temporal (§5.1.7): Wallclock unconditionally heap (header with instant), Local delegates to inner.
        KaiType::Temporal { inner, origin, .. } => match origin {
            kai_tast::TemporalOrigin::Wallclock => true,
            kai_tast::TemporalOrigin::Local => heap_bearing(ctx, inner),
        },
        KaiType::Closure { .. } => true,
        _ => false,
    }
}

/// Sanitized helper-name stem for a type (`int32`, `Point`, `string_arr_`).
pub(crate) fn type_stem(ctx: &Ctx<'_>, ty: &KaiType) -> String {
    match ty {
        KaiType::Struct(id) => {
            let llvm = ctx.structs[id.0 as usize].to_string();
            // `%module.Name = type { .. }` -> `module.Name`
            let raw = llvm
                .split('%')
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .unwrap_or("struct");
            raw.replace('"', "")
        }
        other => other.to_string(),
    }
}

/// Releases ONE value of `ty` held in `value_ptr` — a slot pointing at
/// storage of `ty`: strings/arrays keep a header POINTER in the slot;
/// structs ARE the aggregate in the slot.
pub(crate) fn emit_release_slot<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: &KaiType,
    value_ptr: inkwell::values::PointerValue<'ctx>,
) {
    match ty {
        KaiType::Temporal { inner, origin, .. } => match origin {
            kai_tast::TemporalOrigin::Wallclock => {
                // §5.1.7 two-step release lives inside the generated payload
                // dtor (cascade into heap-bearing inner at rc==0), then the
                // header itself frees — mirroring array + ElemDtor (§9.9).
                let hdr = ctx
                    .builder
                    .build_load(
                        ctx.context.ptr_type(Default::default()),
                        value_ptr,
                        "wallclock.hdr",
                    )
                    .expect("load wallclock header");
                call_void(
                    ctx,
                    crate::runtime::wallclock::wallclock_release_fn(ctx),
                    &[hdr],
                );
            }
            kai_tast::TemporalOrigin::Local => {
                emit_release_slot(ctx, inner, value_ptr);
            }
        },
        KaiType::String | KaiType::Array(_) => {
            let hdr = ctx
                .builder
                .build_load(
                    ctx.context.ptr_type(Default::default()),
                    value_ptr,
                    "rel.hdr",
                )
                .expect("load header for release");
            call_void(ctx, crate::runtime::release_fn(ctx), &[hdr]);
        }
        KaiType::Struct(_) if heap_bearing(ctx, ty) => {
            call_void(ctx, struct_helper(ctx, ty, Helper::Release), &[value_ptr.into()]);
        }
        // Tagged unions (§9.9a): generated helper checks the tag and
        // releases only the ACTIVE payload slot.
        KaiType::Optional(_) | KaiType::Result { .. } => {
            call_void(ctx, tagged_helper(ctx, ty, Helper::Release), &[value_ptr.into()]);
        }
        // A closure slot holds `{ code, env }`: releasing means dropping one
        // reference to the ENVIRONMENT header — its dtor cascades into the
        // captured values exactly once, when rc reaches zero (§9.10).
        KaiType::Closure { .. } => {
            let env = ctx
                .builder
                .build_load(
                    ctx.context.ptr_type(Default::default()),
                    member_gep(ctx, value_ptr, 1, "clo.env.p"),
                    "clo.env",
                )
                .expect("load env for release");
            release_header_value(ctx, env);
        }
        _ => {}
    }
}

/// `%KaiFat{code,env}` second-member GEP on a POINTER to the aggregate.
fn member_gep<'ctx>(
    ctx: &Ctx<'ctx>,
    place: inkwell::values::PointerValue<'ctx>,
    idx: u32,
    name: &str,
) -> inkwell::values::PointerValue<'ctx> {
    let fat = crate::types::closure_fat_ty(ctx);
    ctx.builder
        .build_struct_gep(fat, place, idx, name)
        .expect("fat gep")
}

/// Releases one header POINTER value directly — the for-end of an owned
/// temporary iterable.
pub(crate) fn release_header_value<'ctx>(ctx: &Ctx<'ctx>, value: BasicValueEnum<'ctx>) {
    call_void(
        ctx,
        crate::runtime::release_fn(ctx),
        &[value.into_pointer_value().into()],
    );
}

/// Retains one borrowed HEADER value (string or array). The value flows
/// through unchanged.
pub(crate) fn retain_header<'ctx>(ctx: &Ctx<'ctx>, value: BasicValueEnum<'ctx>) {
    call_void(
        ctx,
        crate::runtime::retain_fn(ctx),
        &[value.into_pointer_value().into()],
    );
}

/// Retains each heap-bearing field of the aggregate AT `place`, then loads
/// and returns the (bitwise-copied) aggregate — §9.5 copy semantics.
pub(crate) fn retain_struct_copy<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: &KaiType,
    place: inkwell::values::PointerValue<'ctx>,
) -> BasicValueEnum<'ctx> {
    call_void(ctx, struct_helper(ctx, ty, Helper::Retain), &[place.into()]);
    ctx.builder
        .build_load(crate::types::to_llvm(ctx, ty), place, "copied")
        .expect("load copied aggregate")
}

#[derive(Clone, Copy)]
enum Helper {
    Retain,
    Release,
}

impl Helper {
    fn prefix(self) -> &'static str {
        match self {
            Helper::Retain => "kai.retain",
            Helper::Release => "kai.release",
        }
    }
}

/// `void @kai.<retain|release>.<S>(ptr agg)` — per-field recursion point.
fn struct_helper<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType, which: Helper) -> FunctionValue<'ctx> {
    let id = match ty {
        KaiType::Struct(id) => *id,
        other => unreachable!("struct helper for {other:?}"),
    };
    let key = format!("{}@{}", which.prefix(), type_stem(ctx, ty));
    let cached = match which {
        Helper::Retain => ctx.retain_helpers.borrow().get(&key).copied(),
        Helper::Release => ctx.release_helpers.borrow().get(&key).copied(),
    };
    if let Some(existing) = cached {
        return existing;
    }

    let name = sanitize_symbol(&key);
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let function = ctx.module.add_function(&name, llvm, None);
    match which {
        Helper::Retain => ctx.retain_helpers.borrow_mut().insert(key, function),
        Helper::Release => ctx.release_helpers.borrow_mut().insert(key, function),
    };

    // Generate the body, recursing into field types. The builder belongs
    // to whatever code is being emitted right now — save and restore it.
    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let fields = ctx.struct_fields[id.0 as usize].clone();
    let aggregate = function
        .get_nth_param(0)
        .expect("agg param")
        .into_pointer_value();
    for (idx, field_ty) in fields.iter().enumerate() {
        if !heap_bearing(ctx, field_ty) {
            continue;
        }
        let field_slot = super::field_gep(ctx, id, aggregate, idx as u32, "fld");
        match which {
            Helper::Retain => {
                let loaded = ctx
                    .builder
                    .build_load(crate::types::to_llvm(ctx, field_ty), field_slot, "fld.v")
                    .expect("field load");
                match field_ty {
                    KaiType::String | KaiType::Array(_) => retain_header(ctx, loaded),
                    // A closure field retains through its ENV member.
                    KaiType::Closure { .. } => retain_header(ctx, loaded),
                    // Nested aggregates recurse on the field's storage.
                    other @ KaiType::Struct(_) => {
                        retain_struct_copy(ctx, other, field_slot);
                    }
                    other @ (KaiType::Optional(_) | KaiType::Result { .. }) => {
                        retain_tagged_copy(ctx, other, field_slot);
                    }
                    _ => {}
                }
            }
            Helper::Release => emit_release_slot(ctx, field_ty, field_slot),
        }
    }
    let _ = ctx.builder.build_return(None);

    if let Some(saved) = saved_block {
        ctx.builder.position_at_end(saved);
    }
    function
}

/// Element destructor for arrays whose elements are heap-bearing:
/// `void @kai.dtor.elems.<elem>(ptr hdr)` — loops every element and
/// releases it. Returns None for scalar element types (dtor = null).
pub(crate) fn ensure_elem_dtor<'ctx>(
    ctx: &Ctx<'ctx>,
    elem_ty: &KaiType,
) -> Option<FunctionValue<'ctx>> {
    if !heap_bearing(ctx, elem_ty) {
        return None;
    }
    let key = format!("kai.dtor.elems@{}", type_stem(ctx, elem_ty));
    if let Some(existing) = ctx.elem_dtors.borrow().get(&key).copied() {
        return Some(existing);
    }

    let name = sanitize_symbol(&key);
    let ptr = ctx.context.ptr_type(Default::default());
    let i64_ty = ctx.context.i64_type();
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let function = ctx.module.add_function(&name, llvm, None);
    ctx.elem_dtors.borrow_mut().insert(key, function);

    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    // Shape key only — element loads use the concrete elem LLVM type.
    let elem_llvm = crate::types::to_llvm(ctx, elem_ty);
    let header_ty = crate::runtime::array_header_ty(ctx, &elem_llvm.to_string());
    let hdr = function.get_nth_param(0).expect("hdr param").into_pointer_value();

    let len_slot = ctx
        .builder
        .build_struct_gep(header_ty, hdr, 1, "dtor.len.p")
        .expect("len gep");
    let len = ctx
        .builder
        .build_load(i64_ty, len_slot, "dtor.len")
        .expect("len load")
        .into_int_value();
    let elems = expr::elems_storage_of(ctx, hdr, elem_llvm);

    let loop_bb = ctx.context.append_basic_block(function, "dtor.loop");
    let body_bb = ctx.context.append_basic_block(function, "dtor.body");
    let done_bb = ctx.context.append_basic_block(function, "dtor.done");
    let idx_slot = ctx.builder.build_alloca(i64_ty, "dtor.i").expect("i alloca");
    let _ = ctx.builder.build_store(idx_slot, i64_ty.const_zero());
    let _ = ctx.builder.build_unconditional_branch(loop_bb);

    ctx.builder.position_at_end(loop_bb);
    let i = ctx
        .builder
        .build_load(i64_ty, idx_slot, "dtor.i.v")
        .expect("i load")
        .into_int_value();
    let more = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::SLT, i, len, "dtor.more")
        .expect("icmp");
    let _ = ctx.builder.build_conditional_branch(more, body_bb, done_bb);

    ctx.builder.position_at_end(body_bb);
    let slot = unsafe {
        ctx.builder
            .build_in_bounds_gep(elem_llvm, elems, &[i], "dtor.elem")
            .expect("elem gep")
    };
    emit_release_slot(ctx, elem_ty, slot);
    let next = ctx
        .builder
        .build_int_add(i, i64_ty.const_int(1, false), "dtor.next")
        .expect("iadd");
    let _ = ctx.builder.build_store(idx_slot, next);
    let _ = ctx.builder.build_unconditional_branch(loop_bb);

    ctx.builder.position_at_end(done_bb);
    let _ = ctx.builder.build_return(None);

    if let Some(saved) = saved_block {
        ctx.builder.position_at_end(saved);
    }
    Some(function)
}

pub(crate) fn call_void<'ctx>(
    ctx: &Ctx<'ctx>,
    function: FunctionValue<'ctx>,
    args: &[BasicValueEnum<'ctx>],
) {
    let args_meta: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
        args.iter().map(|a| (*a).into()).collect();
    let _site = ctx
        .builder
        .build_call(function, &args_meta, "own")
        .expect("ownership call");
}

pub(crate) fn sanitize_symbol(stem: &str) -> String {
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

// -- v0.0.6 tagged unions & closures (§9.9a/§9.10) ------------------------------

/// Retains each heap-bearing ACTIVE-payload field of the tagged aggregate at
/// `place`, then loads and returns the bitwise copy — §9.5 copy semantics
/// generalized behind a runtime tag check.
pub(crate) fn retain_tagged_copy<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: &KaiType,
    place: inkwell::values::PointerValue<'ctx>,
) -> BasicValueEnum<'ctx> {
    call_void(ctx, tagged_helper(ctx, ty, Helper::Retain), &[place.into()]);
    ctx.builder
        .build_load(crate::types::to_llvm(ctx, ty), place, "copied")
        .expect("load copied aggregate")
}

/// `void @kai.<retain|release>.<opt|res>.<T>(ptr agg)` — the tag decides
/// which payload slot the per-field machinery touches. One helper per
/// instantiated shape; the tag itself owns nothing (§9.9a).
fn tagged_helper<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType, which: Helper) -> FunctionValue<'ctx> {
    let key = format!("{}@{}", which.prefix(), type_stem(ctx, ty));
    let cached = match which {
        Helper::Retain => ctx.retain_helpers.borrow().get(&key).copied(),
        Helper::Release => ctx.release_helpers.borrow().get(&key).copied(),
    };
    if let Some(existing) = cached {
        return existing;
    }

    let name = sanitize_symbol(&key);
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let function = ctx.module.add_function(&name, llvm, None);
    match which {
        Helper::Retain => ctx.retain_helpers.borrow_mut().insert(key, function),
        Helper::Release => ctx.release_helpers.borrow_mut().insert(key, function),
    };

    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let agg = function
        .get_nth_param(0)
        .expect("agg param")
        .into_pointer_value();
    let llvm_ty = crate::types::to_llvm(ctx, ty).into_struct_type();

    // Payload slot indices: Optional = {tag, payload}; Result = {tag, ok, err}.
    let (tag_idx, branches): (u32, Vec<(u64, &KaiType, u32)>) = match ty {
        KaiType::Optional(inner) => (0, vec![(0, inner.as_ref(), 1)]),
        KaiType::Result { ok, err } => (
            0,
            vec![(0, ok.as_ref(), 1), (1, err.as_ref(), 2)],
        ),
        other => unreachable!("tagged helper for {other:?}"),
    };

    let i64_ty = ctx.context.i64_type();
    let tag_slot = ctx
        .builder
        .build_struct_gep(llvm_ty, agg, tag_idx, "tag.p")
        .expect("tag gep");
    let tag = ctx
        .builder
        .build_load(i64_ty, tag_slot, "tag")
        .expect("tag load")
        .into_int_value();
    let done = ctx.context.append_basic_block(function, "inactive");

    for (value, payload_ty, payload_idx) in branches {
        if !heap_bearing(ctx, payload_ty) {
            continue; // stack payloads: no RC ops in any branch (compile-time keyed)
        }
        let is_active = ctx
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag,
                i64_ty.const_int(value, false),
                "active",
            )
            .expect("tag cmp");
        let branch = ctx.context.append_basic_block(function, "active.payload");
        let _ = ctx.builder.build_conditional_branch(is_active, branch, done);

        ctx.builder.position_at_end(branch);
        let slot = ctx
            .builder
            .build_struct_gep(llvm_ty, agg, payload_idx, "payload.p")
            .expect("payload gep");
        emit_payload_op(ctx, payload_ty, which, slot);
        let _ = ctx.builder.build_unconditional_branch(done);
    }
    ctx.builder.position_at_end(done);
    let _ = ctx.builder.build_return(None);

    if let Some(saved) = saved_block {
        ctx.builder.position_at_end(saved);
    }
    function
}

/// One payload-slot operation inside a tagged helper: headers retain/release
/// through the pointer stored in the slot; aggregates recurse on storage.
fn emit_payload_op<'ctx>(
    ctx: &Ctx<'ctx>,
    payload_ty: &KaiType,
    which: Helper,
    slot: inkwell::values::PointerValue<'ctx>,
) {
    match (which, payload_ty) {
        (Helper::Retain, KaiType::String | KaiType::Array(_) | KaiType::Closure { .. }) => {
            let loaded = ctx
                .builder
                .build_load(crate::types::to_llvm(ctx, payload_ty), slot, "p.v")
                .expect("payload load");
            retain_header(ctx, loaded);
        }
        (Helper::Retain, other @ (KaiType::Struct(_) | KaiType::Optional(_) | KaiType::Result { .. })) => {
            if matches!(other, KaiType::Struct(_)) {
                retain_struct_copy(ctx, other, slot);
            } else {
                retain_tagged_copy(ctx, other, slot);
            }
        }
        // Stack payloads never reach here: the helper skips non-heap
        // branches at generation time.
        (Helper::Release, _) => emit_release_slot(ctx, payload_ty, slot),
        (Helper::Retain, _) => {}
    }
}
