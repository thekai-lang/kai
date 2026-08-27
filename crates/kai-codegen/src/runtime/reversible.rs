//! Reversible ledger host (§5.3). Split from `runtime/mod.rs` (§8.4 LOC
//! discipline); mirrors `observe.rs`/`wallclock.rs` as a self-contained
//! submodule.
//!
//! The ledger is a LIFO stack of activations, one per `reversible` function
//! call (§5.3.5). Each activation's entries are the pre-mutation Place
//! snapshots pushed by `kai_reversible_push`. A snapshot is the OWNING copy
//! of the OLD value: the push `retain(old)` first, so the ledger holds a real
//! refcount claim — never a bare pointer. `dtor` (null for non-heap types) is
//! the codegen-generated per-type release for one value; the host stays dumb
//! and just calls it for the displaced current (unwind) or the snapshot
//! (commit).

use crate::context::Ctx;
use crate::runtime::get_or_declare;
use inkwell::values::FunctionValue;
use std::cell::RefCell;

type ReversibleDtor = extern "C" fn(*mut u8);

struct ReversibleEntry {
    place: *mut u8,
    snapshot: Vec<u8>,
    dtor: Option<ReversibleDtor>,
}

thread_local! {
    static REVERSE_STACK: RefCell<Vec<Vec<ReversibleEntry>>> = RefCell::new(Vec::new());
}

/// Enter a new `reversible` activation — pushes an empty ledger for this call.
#[unsafe(no_mangle)]
pub extern "C" fn kai_reversible_enter() {
    REVERSE_STACK.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Push a Place snapshot onto the current activation's ledger.
/// `place` points at the Place's storage (slot). `snapshot_ptr` points at a
/// stack slot holding the OLD value — already retained if heap-bearing.
/// `size` is the byte size of the snapshot value. `dtor` releases the heap
/// claims of ONE value of the snapshot's type (null for non-heap types); the
/// snapshot's claim is transferred to the Place on unwind, so `dtor` is only
/// ever invoked on the displaced CURRENT value (unwind) or the snapshot's
/// own claim (commit).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_reversible_push(
    place: *mut u8,
    snapshot_ptr: *const u8,
    size: u64,
    dtor: *const (),
) {
    if place.is_null() || snapshot_ptr.is_null() {
        return;
    }
    let sz = usize::try_from(size).unwrap_or(0);
    if sz == 0 {
        return;
    }
    let mut buf = vec![0u8; sz];
    unsafe { std::ptr::copy_nonoverlapping(snapshot_ptr, buf.as_mut_ptr(), sz) };
    let entry = ReversibleEntry {
        place,
        snapshot: buf,
        dtor: (!dtor.is_null()).then(|| {
            // SAFETY: `dtor` is the codegen-generated release thunk when
            // non-null, cast to the typed fn pointer form.
            unsafe { std::mem::transmute::<*const (), ReversibleDtor>(dtor) }
        }),
    };
    REVERSE_STACK.with(|s| {
        if let Some(top) = s.borrow_mut().last_mut() {
            top.push(entry);
        }
    });
}

/// Commit: release each snapshot's heap claim (its own retained ref) and pop
/// the ledger. Called on normal return from a `reversible` function — the
/// Place now owns the NEW value, and the OLD snapshot refs are freed.
#[unsafe(no_mangle)]
pub extern "C" fn kai_reversible_commit() {
    let entries = REVERSE_STACK.with(|s| s.borrow_mut().pop());
    let Some(entries) = entries else { return; };
    for e in entries {
        if let Some(dtor) = e.dtor {
            // Release the ledger's retain on the OLD value.
            dtor(e.snapshot.as_ptr() as *mut u8);
        }
    }
}

/// Unwind: walk the current activation's ledger in LIFO order, restoring each
/// Place and releasing the displaced CURRENT value. The snapshot's retain
/// transfers to the Place (it becomes the live value); the ledger is then
/// popped, so the snapshot claim is never double-released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_reversible_unwind() {
    let entries = REVERSE_STACK.with(|s| s.borrow_mut().pop());
    let Some(entries) = entries else { return; };
    for e in entries.into_iter().rev() {
        let sz = e.snapshot.len();
        if sz == 0 || e.place.is_null() {
            continue;
        }
        // Save the current bytes at Place (e.g. "B").
        let mut cur = vec![0u8; sz];
        unsafe { std::ptr::copy_nonoverlapping(e.place, cur.as_mut_ptr(), sz) };
        // Restore the snapshot to Place (e.g. "A").
        unsafe { std::ptr::copy_nonoverlapping(e.snapshot.as_ptr(), e.place, sz) };
        // Release the displaced current value's claim.
        if let Some(dtor) = e.dtor {
            dtor(cur.as_mut_ptr());
        }
        // Snapshot's retain now lives in the Place; do not release it here.
    }
}

pub(crate) fn reversible_enter_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    get_or_declare(ctx, "kai_reversible_enter", ctx.context.void_type().fn_type(&[], false))
}
pub(crate) fn reversible_push_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let i64_ty = ctx.context.i64_type();
    get_or_declare(
        ctx,
        "kai_reversible_push",
        ctx.context.void_type().fn_type(
            &[ptr.into(), ptr.into(), i64_ty.into(), ptr.into()],
            false,
        ),
    )
}
pub(crate) fn reversible_commit_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    get_or_declare(ctx, "kai_reversible_commit", ctx.context.void_type().fn_type(&[], false))
}
pub(crate) fn reversible_unwind_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    get_or_declare(ctx, "kai_reversible_unwind", ctx.context.void_type().fn_type(&[], false))
}
