//! `@wallclock` runtime (§5.1.7): header layout, construction, and release.
//! The instant is a compact i64 (UTC microseconds since epoch), never an
//! RFC 3339 string — that is the §5.1.5 wire format only. The payload lives
//! INLINE after the fixed prefix; the generated payload dtor cascades into a
//! heap-bearing inner exactly once at rc==0 (array + ElemDtor precedent).

use inkwell::values::FunctionValue;

use super::{get_or_declare, oom_panic, Ctx};

// -- v0.0.7 wallclock (§5.1.7) -------------------------------------------------

/// Releases the payload of a dying `@wallclock` header. Generated per inner
/// type by codegen (mirrors `ElemDtor` for arrays); receives the header base.
pub type WallclockDtor = unsafe extern "C" fn(*mut u8);

/// Fixed prefix of an `@wallclock` header (§5.1.7). The payload is stored
/// INLINE after this prefix, at byte offset 32 — the LLVM-side named type
/// `%KaiWallclock.<inner> = { i64 rc, i64 instant, ptr dtor, i64 nbytes,
/// <inner> payload }` must keep matching (payload = GEP index 4).
#[repr(C)]
pub struct KaiWallclockHeader {
    pub rc: i64,
    /// UTC microseconds since Unix epoch — compact integer, NOT RFC 3339
    /// (that's the §5.1.5 wire format only).
    pub instant: i64,
    pub dtor: Option<WallclockDtor>,
    /// Inline-payload byte count, recorded so release deallocs exactly what
    /// construction allocated.
    pub nbytes: i64,
}

/// Byte offset of the inline payload inside an `@wallclock` header.
pub const WALLCLOCK_PAYLOAD_OFFSET: usize = std::mem::size_of::<KaiWallclockHeader>();
/// Alignment used for every `@wallclock` allocation (covers i64/f64 payloads).
const WALLCLOCK_ALIGN: usize = 16;

fn alloc_aligned(byte_len: usize, align: usize) -> *mut u8 {
    if byte_len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = match std::alloc::Layout::from_size_align(byte_len, align) {
        Ok(layout) => layout,
        Err(_) => oom_panic(),
    };
    // SAFETY: size non-zero with valid alignment.
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        oom_panic();
    }
    ptr
}

/// Current instant in UTC microseconds since the Unix epoch — the value
/// baked into every new `@wallclock` header (§5.1: "re-verified every use";
/// the expiry CHECK itself is still-open codegen work, this only supplies
/// the timestamp the check will read).
#[unsafe(no_mangle)]
pub extern "C" fn kai_wallclock_now() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_micros() as i64,
        Err(_) => 0,
    }
}

/// Allocates an `@wallclock` header with `nbytes` of inline payload storage
/// (uninitialized — the caller stores the inner value immediately through
/// the payload GEP). Refcount starts at 1.
#[unsafe(no_mangle)]
pub extern "C" fn kai_wallclock_new(
    instant: i64,
    dtor: Option<WallclockDtor>,
    nbytes: i64,
) -> *mut u8 {
    let extra = usize::try_from(nbytes.max(0)).unwrap_or(0);
    let total = WALLCLOCK_PAYLOAD_OFFSET + extra;
    let base = alloc_aligned(total, WALLCLOCK_ALIGN);
    // SAFETY: base covers at least the fixed prefix.
    unsafe {
        (*base.cast::<KaiWallclockHeader>()) = KaiWallclockHeader {
            rc: 1,
            instant,
            dtor,
            nbytes,
        };
    }
    base
}

/// Relinquishes one ownership claim on an `@wallclock` header. At the last
/// release the generated payload destructor runs first (cascading into a
/// heap-bearing inner, §5.1.7 two-step), then the header itself is freed.
///
/// # Safety
/// `base` must be a `kai_wallclock_new` pointer or null, and the caller must
/// own one reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_wallclock_release(base: *mut u8) {
    if base.is_null() {
        return;
    }
    // SAFETY: valid wallclock header per the contract.
    let hdr = unsafe { &mut *(base as *mut KaiWallclockHeader) };
    if hdr.rc <= 0 {
        eprintln!("kai runtime error: refcount underflow (double release)");
        std::process::abort();
    }
    hdr.rc -= 1;
    if hdr.rc > 0 {
        return;
    }
    if let Some(dtor) = hdr.dtor {
        // SAFETY: generated dtors accept exactly this header shape.
        unsafe { dtor(base) };
    }
    let extra = usize::try_from(hdr.nbytes.max(0)).unwrap_or(0);
    // SAFETY: allocated in kai_wallclock_new with this exact size/alignment.
    unsafe {
        std::alloc::dealloc(
            base,
            std::alloc::Layout::from_size_align_unchecked(
                WALLCLOCK_PAYLOAD_OFFSET + extra,
                WALLCLOCK_ALIGN,
            ),
        )
    };
}


// -- LLVM-side declarations ----------------------------------------------------

/// `kai_wallclock_now() -> i64` — UTC microseconds since epoch (§5.1.7).
pub(crate) fn wallclock_now_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let llvm = ctx.context.i64_type().fn_type(&[], false);
    get_or_declare(ctx, "kai_wallclock_now", llvm)
}

/// `kai_wallclock_new(instant, dtor, nbytes) -> ptr` — allocates the header.
pub(crate) fn wallclock_new_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let i64_ty = ctx.context.i64_type();
    let llvm = ptr.fn_type(
        &[i64_ty.into(), ptr.into(), i64_ty.into()],
        false,
    );
    get_or_declare(ctx, "kai_wallclock_new", llvm)
}

/// `kai_wallclock_release(ptr)` — rc drop; dtor + free at zero.
pub(crate) fn wallclock_release_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    get_or_declare(ctx, "kai_wallclock_release", llvm)
}
