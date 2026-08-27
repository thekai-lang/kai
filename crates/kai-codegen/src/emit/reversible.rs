//! Reversible snapshots (§5.3): the codegen half of the ledger.
//!
//! Ownership inserted a `ReversiblePush{root, path, ty}` marker immediately
//! before every Place mutation inside a `reversible` function. This module
//! turns that marker into the §5.3 snapshot: resolve the Place to its slot
//! (same `root`/`path` the following `Assign` mutates), load the OLD value,
//! RETAIN it so the ledger owns a real refcount claim (never a bare pointer
//! — §5.3 audit), then call `kai_reversible_push` to append to the current
//! activation's ledger.
//!
//! On unwind/commit the HOST runtime walks that ledger generically; the only
//! type knowledge it needs is "how to release one value of `ty`", which this
//! module supplies as a per-type dtor thunk (`snapshot_dtor`). The host stays
//! dumb and just calls it. Split from `stmt.rs` to honor §8.4 LOC discipline.

use crate::context::Ctx;
use crate::emit::ownership;
use crate::frame::Frame;
use inkwell::types::BasicType;
use inkwell::values::FunctionValue;
use kai_tast::{KaiType};

/// Emits `kai_reversible_commit` (release this activation's snapshot claims
/// before leaving) when `frame` is inside a `reversible` function (§5.3).
/// Called at every return site — statement `return`, `ReturnCleanup`, and
/// dead-code fallback returns.
pub(crate) fn commit_if_reversible<'ctx>(ctx: &Ctx<'ctx>, frame: &Frame<'ctx>) {
    if frame.reversible {
        ctx.builder
            .build_call(crate::runtime::reversible_commit_fn(ctx), &[], "rev.commit")
            .expect("kai_reversible_commit call");
    }
}

/// Emits `kai_reversible_unwind` (roll back the current activation's ledger —
/// restore every Place + release displaced values) when the current function
/// is a `reversible` one. Called at every runtime-panic site (bounds checks,
/// arith traps, §5.2 require) before the terminal §10.1 `kai_panic`.
pub(crate) fn unwind_if_active<'ctx>(ctx: &Ctx<'ctx>) {
    if ctx.reversible_active.get() {
        ctx.builder
            .build_call(crate::runtime::reversible_unwind_fn(ctx), &[], "rev.unwind")
            .expect("kai_reversible_unwind call");
    }
}

/// Emits the §5.3.1 snapshot for one `ReversiblePush`.
///
/// `user` audit (pre-E): `root` + `path` fully determine the slot via the
/// same `resolve_place` used by the following `Assign`, so snapshot and store
/// hit the SAME element — index expressions (pure in Kai) re-evaluate to the
/// same slot with no intervening store. The ledger stores this resolved slot
/// pointer, so UNWIND never re-evaluates a path expression.
pub(crate) fn emit_push_inline<'ctx>(
    ctx: &Ctx<'ctx>,
    place: inkwell::values::PointerValue<'ctx>,
    ty: &kai_tast::KaiType,
) {
    // Snapshot buffer holds the OLD value WITH its heap claims retained.
    let snap_ty = crate::types::to_llvm(ctx, ty);
    let snapshot_slot =
        crate::emit::alloca_in_entry(ctx, crate::emit::current_function(ctx), snap_ty, "rev.snap");
    let retained = ownership::emit_retain_slot(ctx, ty, place);
    let _ = ctx.builder.build_store(snapshot_slot, retained);

    let size = snap_ty.size_of().expect("reversible value is sized");
    let dtor = snapshot_dtor(ctx, ty);

    let args = [
        place.into(),
        snapshot_slot.into(),
        size.into(),
        dtor.into(),
    ];
    ctx.builder
        .build_call(crate::runtime::reversible_push_fn(ctx), &args, "rev.push")
        .expect("kai_reversible_push call");
}

/// Returns a null pointer for non-heap `ty`, else the cached per-type
/// snapshot-release thunk: `void @kai.snapREL.<stem>(ptr value)` that calls
/// `emit_release_slot` on one value of `ty`. The host ledger invokes it on
/// the displaced current value (unwind) or the snapshot's own claim (commit).
pub(crate) fn snapshot_dtor<'ctx>(
    ctx: &Ctx<'ctx>,
    ty: &KaiType,
) -> inkwell::values::PointerValue<'ctx> {
    let null_ptr = ctx.context.ptr_type(Default::default()).const_zero();
    if !ownership::heap_bearing(ctx, ty) {
        return null_ptr;
    }
    let key = format!("kai.snapREL@{}", ownership::type_stem(ctx, ty));
    if let Some(existing) = ctx.snapshot_dtors.borrow().get(&key).copied() {
        return existing.as_global_value().as_pointer_value();
    }

    let name = ownership::sanitize_symbol(&key);
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let function: FunctionValue<'ctx> = ctx.module.add_function(&name, llvm, None);
    ctx.snapshot_dtors.borrow_mut().insert(key, function);

    let saved_active = ctx.reversible_active.replace(false);
    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);
    let value = function
        .get_nth_param(0)
        .expect("value param")
        .into_pointer_value();
    ownership::emit_release_slot(ctx, ty, value);
    ctx.builder.build_return(None).expect("return");
    if let Some(block) = saved_block {
        ctx.builder.position_at_end(block);
    }
    ctx.reversible_active.set(saved_active);

    function.as_global_value().as_pointer_value()
}
