//! Tagged-union (§9.9a) ownership helpers: `retain_tagged_copy`,
//! `tagged_helper`, and the per-payload dispatch. Split from `ownership.rs`
//! (§8 LOC discipline); these generalize the per-field pattern behind a
//! runtime tag check — the tag itself owns nothing.

use super::ownership::{
    call_void, emit_release_slot, heap_bearing, retain_header, retain_struct_copy, sanitize_symbol,
    type_stem, Helper,
};
use crate::context::Ctx;
use inkwell::values::{BasicValueEnum, FunctionValue};
use kai_tast::KaiType;

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
pub(crate) fn tagged_helper<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType, which: Helper) -> FunctionValue<'ctx> {
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

    // v0.0.8.4: chain one check per heap-bearing payload. Each icmp must be
    // emitted into a block that does not already carry a terminator — the
    // previous code parked the builder at the end of the LAST active.payload
    // block, so a SECOND heap-bearing branch (Result<string,string> etc.)
    // inserted its conditional branch after an existing terminator and
    // aborted codegen ("Terminator found in the middle of a basic block").
    // Latent since v0.0.6: Optional has one branch, Result with a single
    // heap-bearing side skips the other via `continue`, both-heap Result
    // was simply never emitted until ownership-pass hoisting produced a
    // hidden local of that shape.
    let heap_branches: Vec<(u64, &KaiType, u32)> = branches
        .iter()
        .filter(|(_, payload_ty, _)| heap_bearing(ctx, payload_ty))
        .copied()
        .collect();
    let mut current = entry;
    let last_idx = heap_branches.len().saturating_sub(1);
    for (i, (value, payload_ty, payload_idx)) in heap_branches.iter().enumerate() {
        ctx.builder.position_at_end(current);
        let is_active = ctx
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag,
                i64_ty.const_int(*value, false),
                "active",
            )
            .expect("tag cmp");
        let fallthrough = if i == last_idx {
            done
        } else {
            ctx.context.append_basic_block(function, "tag.check")
        };
        let branch = ctx.context.append_basic_block(function, "active.payload");
        let _ = ctx.builder.build_conditional_branch(is_active, branch, fallthrough);

        ctx.builder.position_at_end(branch);
        let slot = ctx
            .builder
            .build_struct_gep(llvm_ty, agg, *payload_idx, "payload.p")
            .expect("payload gep");
        emit_payload_op(ctx, payload_ty, which, slot);
        let _ = ctx.builder.build_unconditional_branch(done);
        current = fallthrough;
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
