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
use std::alloc::{Layout, alloc, dealloc};

type ReversibleDtor = extern "C" fn(*mut u8);

struct AlignedBuffer {
    ptr: *mut u8,
    layout: Layout,
}

impl AlignedBuffer {
    fn new(size: usize, align: usize) -> Self {
        if size == 0 {
            return Self { ptr: std::ptr::null_mut(), layout: Layout::from_size_align(0, 1).unwrap() };
        }
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { alloc(layout) };
        Self { ptr, layout }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.layout.size() > 0 {
            unsafe { dealloc(self.ptr, self.layout) };
        }
    }
}

enum ReversibleEntry {
    Snapshot {
        place: *mut u8,
        snapshot: Vec<u8>,
        dtor: Option<ReversibleDtor>,
    },
    Compensate {
        env: AlignedBuffer,
        thunk: extern "C" fn(*mut u8),
        release: Option<ReversibleDtor>,
    },
}

thread_local! {
    static REVERSE_STACK: RefCell<Vec<Vec<ReversibleEntry>>> = RefCell::new(Vec::new());
}

/// Enter a new `reversible` activation — pushes an empty ledger for this call.
#[unsafe(no_mangle)]
pub extern "C" fn kai_reversible_enter() {
    REVERSE_STACK.with(|s| s.borrow_mut().push(Vec::new()));
}

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
    let entry = ReversibleEntry::Snapshot {
        place,
        snapshot: buf,
        dtor: (!dtor.is_null()).then(|| {
            unsafe { std::mem::transmute::<*const (), ReversibleDtor>(dtor) }
        }),
    };
    REVERSE_STACK.with(|s| {
        if let Some(top) = s.borrow_mut().last_mut() {
            top.push(entry);
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_reversible_push_compensate(
    env_ptr: *const u8,
    env_size: u64,
    thunk: *const (),
    release: *const (),
) {
    if thunk.is_null() {
        return;
    }
    let sz = usize::try_from(env_size).unwrap_or(0);
    // Align to 16 bytes minimum, enough for all LLVM types in Kai (ptr, struct, vector).
    let mut buf = AlignedBuffer::new(sz, 16);
    if sz > 0 && !env_ptr.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(env_ptr, buf.ptr, sz) };
    }
    let entry = ReversibleEntry::Compensate {
        env: buf,
        thunk: unsafe { std::mem::transmute::<*const (), extern "C" fn(*mut u8)>(thunk) },
        release: (!release.is_null()).then(|| {
            unsafe { std::mem::transmute::<*const (), ReversibleDtor>(release) }
        }),
    };
    REVERSE_STACK.with(|s| {
        if let Some(top) = s.borrow_mut().last_mut() {
            top.push(entry);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kai_reversible_commit() {
    let entries = REVERSE_STACK.with(|s| s.borrow_mut().pop());
    let Some(entries) = entries else { return; };
    for e in entries {
        match e {
            ReversibleEntry::Snapshot { snapshot, dtor, .. } => {
                if let Some(dtor) = dtor {
                    dtor(snapshot.as_ptr() as *mut u8);
                }
            }
            ReversibleEntry::Compensate { env, release, .. } => {
                if let Some(release_fn) = release {
                    release_fn(env.ptr);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_reversible_unwind() {
    let entries = REVERSE_STACK.with(|s| s.borrow_mut().pop());
    let Some(entries) = entries else { return; };
    for mut e in entries.into_iter().rev() {
        match &mut e {
            ReversibleEntry::Snapshot { place, snapshot, dtor } => {
                let sz = snapshot.len();
                if sz == 0 || place.is_null() {
                    continue;
                }
                let mut cur = vec![0u8; sz];
                unsafe { std::ptr::copy_nonoverlapping(*place, cur.as_mut_ptr(), sz) };
                unsafe { std::ptr::copy_nonoverlapping(snapshot.as_ptr(), *place, sz) };
                if let Some(dtor) = dtor {
                    dtor(cur.as_mut_ptr());
                }
            }
            ReversibleEntry::Compensate { env, thunk, release } => {
                thunk(env.ptr);
                if let Some(release_fn) = release {
                    release_fn(env.ptr);
                }
            }
        }
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
pub(crate) fn reversible_push_compensate_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let i64_ty = ctx.context.i64_type();
    get_or_declare(
        ctx,
        "kai_reversible_push_compensate",
        ctx.context.void_type().fn_type(
            &[ptr.into(), i64_ty.into(), ptr.into(), ptr.into()],
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
