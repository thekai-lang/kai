//! Host-side runtime for heap values (v0.0.5).
//!
//! Intrinsics are plain Rust symbols exported from this process; MCJIT
//! resolves them by name at link time, so JIT programs call them like any
//! imported function. AOT output (`compile_ir`) carries declarations only.
//!
//! Layouts (§9.1): every heap value is a POINTER to one uniform header.
//! Strings and arrays share the shape so a single generic retain/release
//! works for both; `nbytes` records the payload allocation size so release
//! can dealloc with the exact layout it alloc'd (Rust requires the match),
//! and `dtor` lets arrays of heap-bearing elements release their contents
//! exactly once — when the LAST owner drops the header (§9.9: the array
//! owns its elements).
//!
//! ```text
//! KaiHeap { i64 rc, i64 len, i64 nbytes, ptr payload, ptr dtor }
//! ```
//!
//! Refcounts start at 1 (the creator owns); retain bumps, release drops,
//! zero frees. Element destructors run inside that free path only, so
//! co-owned arrays never double-release elements.

use crate::context::Ctx;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

/// The one heap header. `payload` is the byte blob for strings or the
/// untyped element storage for arrays (callers GEP with static types).
#[repr(C)]
pub struct KaiHeapHeader {
    pub rc: i64,
    /// Authoritative element count (arrays) / byte count (strings).
    pub len: i64,
    /// Exact size of the `payload` allocation, for matching dealloc.
    pub nbytes: i64,
    pub payload: *mut u8,
    /// Array-of-heap-elements destructor; NULL for scalars and strings.
    pub dtor: Option<ElemDtor>,
}

/// Releases every element of an array about to die. Generated per element
/// type by codegen; receives the array header.
pub type ElemDtor = unsafe extern "C" fn(*mut KaiHeapHeader);

fn alloc_bytes(byte_len: usize) -> *mut u8 {
    if byte_len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    // SAFETY: size is non-zero; alignment 1 is always satisfiable.
    let layout = std::alloc::Layout::from_size_align(byte_len, 1)
        .expect("byte layout is representable");
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    ptr
}

/// `"bytes"` -> owned header + copied payload.
///
/// # Safety
/// `data` must be readable for `len` bytes unless `len <= 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_string_new(data: *const u8, len: i64) -> *mut KaiHeapHeader {
    let byte_len = usize::try_from(len).unwrap_or(0);
    let buf = alloc_bytes(byte_len);
    if byte_len > 0 {
        debug_assert!(!data.is_null());
        unsafe { std::ptr::copy_nonoverlapping(data, buf, byte_len) };
    }
    Box::into_raw(Box::new(KaiHeapHeader {
        rc: 1,
        len,
        nbytes: byte_len as i64,
        payload: buf,
        dtor: None,
    }))
}

/// `[..]` -> owned header + zero-initialized element storage. Zero-init so
/// an element slot is never observed uninitialized even on buggy paths.
///
/// `dtor` may be null (scalar elements): nothing per-element to release.
#[unsafe(no_mangle)]
pub extern "C" fn kai_array_new(
    len: i64,
    elem_size: i64,
    dtor: Option<ElemDtor>,
) -> *mut KaiHeapHeader {
    let byte_len = usize::try_from(len).unwrap_or(0).saturating_mul(
        usize::try_from(elem_size).unwrap_or(0),
    );
    let elems = alloc_bytes(byte_len);
    if byte_len > 0 {
        // SAFETY: `elems` covers exactly `byte_len` writable bytes.
        unsafe { std::ptr::write_bytes(elems, 0, byte_len) };
    }
    Box::into_raw(Box::new(KaiHeapHeader {
        rc: 1,
        len,
        nbytes: byte_len as i64,
        payload: elems,
        dtor,
    }))
}

/// Co-ownership: the caller becomes another owner of `value`.
///
/// # Safety
/// `value` must be a valid heap header or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_retain(value: *mut KaiHeapHeader) {
    if value.is_null() {
        return;
    }
    // SAFETY: non-null, valid header per the contract.
    unsafe { (*value).rc += 1 };
}

/// Relinquishes one ownership claim. At the last release the payload and
/// header are freed; array element destructors run exactly here, once.
///
/// # Safety
/// `value` must be a valid heap header or null, and the caller must own a
/// reference (every release site corresponds to one retain/move).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_release(value: *mut KaiHeapHeader) {
    if value.is_null() {
        return;
    }
    // SAFETY: non-null, valid header per the contract.
    let hdr = unsafe { &mut *value };
    hdr.rc -= 1;
    if hdr.rc > 0 {
        return;
    }
    if let Some(dtor) = hdr.dtor {
        // SAFETY: generated dtors accept exactly this header shape.
        unsafe { dtor(value) };
    }
    let n = hdr.nbytes;
    if n > 0 {
        // SAFETY: allocated in alloc_bytes with align 1 and this exact
        // size, recorded in `nbytes` at creation.
        unsafe {
            std::alloc::dealloc(
                hdr.payload,
                std::alloc::Layout::from_size_align_unchecked(n as usize, 1),
            )
        };
    }
    // SAFETY: created by Box::into_raw in *_new.
    unsafe { drop(Box::from_raw(value)) };
}

/// String equality compares CONTENT (§9.7): two independently built strings
/// holding the same bytes are equal; pointer identity is irrelevant. A
/// string is only ever compared against itself via aliasing, where lengths
/// and pointers match trivially.
///
/// Returns 0/1 as u8 — ABI-stable across the JIT boundary (i8 at call sites).
///
/// # Safety
/// Both arguments must be valid Kai string headers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_string_eq(a: *const KaiHeapHeader, b: *const KaiHeapHeader) -> u8 {
    let (a, b) = unsafe { (&*a, &*b) };
    if a.len != b.len {
        return 0;
    }
    let byte_len = usize::try_from(a.len).unwrap_or(0);
    if byte_len == 0 || a.payload == b.payload {
        return 1;
    }
    // SAFETY: both payloads hold at least `byte_len` readable bytes.
    let same = unsafe {
        std::slice::from_raw_parts(a.payload, byte_len)
            == std::slice::from_raw_parts(b.payload, byte_len)
    };
    u8::from(same)
}

/// §10.1: report a runtime violation in the mandated format and exit with
/// code 101. `msg` is length-delimited (baked globals carry no NUL);
/// `file` is a NUL-terminated module-file global the compiler emits beside
/// each call site. Never returns.
///
/// # Safety
/// `msg` must be readable for `msg_len` bytes; `file` must be a valid
/// C string — both are compiler-generated globals.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_panic(
    msg: *const u8,
    msg_len: i64,
    file: *const u8,
    line: i64,
    col: i64,
) {
    use std::io::Write;

    let len = usize::try_from(msg_len).unwrap_or(0);
    let msg = if msg.is_null() || len == 0 {
        &[][..]
    } else {
        // SAFETY: contract above — compiler-generated globals.
        unsafe { std::slice::from_raw_parts(msg, len) }
    };
    let file = if file.is_null() {
        std::borrow::Cow::Borrowed("<unknown>")
    } else {
        // SAFETY: contract above.
        unsafe { std::ffi::CStr::from_ptr(file.cast()) }.to_string_lossy()
    };

    let stderr = std::io::stderr();
    let mut out = stderr.lock();
    let _ = writeln!(out, "kai runtime panic: {}", String::from_utf8_lossy(msg));
    let _ = writeln!(out, "  at {}:{}:{}", file, line, col);
    let _ = out.flush();
    std::process::exit(101);
}

// -- LLVM-side plumbing -------------------------------------------------------

/// `%KaiString` named struct type, created once per module. Unused while
/// strings travel opaque; the ownership phase (retain/release, data access)
/// GEPs through this shape.
#[allow(dead_code)]
pub(crate) fn heap_header_shape<'ctx>(ctx: &Ctx<'ctx>) -> [inkwell::types::BasicTypeEnum<'ctx>; 5] {
    let i64_ty = ctx.context.i64_type().into();
    let ptr = ctx.context.ptr_type(Default::default()).into();
    [i64_ty, i64_ty, i64_ty, ptr, ptr]
}

#[allow(dead_code)]
pub(crate) fn string_header_ty<'ctx>(ctx: &Ctx<'ctx>) -> StructType<'ctx> {
    if let Some(existing) = ctx.module.get_struct_type("KaiString") {
        return existing;
    }
    let ty = ctx.context.opaque_struct_type("KaiString");
    ty.set_body(&heap_header_shape(ctx), false);
    ty
}

/// `%KaiArray.<name>` header type for one element type, created once per
/// module per element shape.
pub(crate) fn array_header_ty<'ctx>(ctx: &Ctx<'ctx>, elem_name: &str) -> StructType<'ctx> {
    let name = format!("KaiArray.{elem_name}");
    if let Some(existing) = ctx.module.get_struct_type(&name) {
        return existing;
    }
    let ty = ctx.context.opaque_struct_type(&name);
    ty.set_body(&heap_header_shape(ctx), false);
    ty
}

fn get_or_declare<'ctx>(ctx: &Ctx<'ctx>, name: &str, llvm: inkwell::types::FunctionType<'ctx>) -> FunctionValue<'ctx> {
    if let Some(existing) = ctx.module.get_function(name) {
        return existing;
    }
    ctx.module.add_function(name, llvm, None)
}

/// `kai_string_new(i8*, i64) -> %KaiString*`
pub(crate) fn string_new_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ptr.fn_type(&[ptr.into(), ctx.context.i64_type().into()], false);
    get_or_declare(ctx, "kai_string_new", llvm)
}

/// `kai_array_new(i64 len, i64 elem_size, ptr dtor) -> %KaiArray.<elem>*`
pub(crate) fn array_new_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let i64_ty = ctx.context.i64_type();
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ptr.fn_type(&[i64_ty.into(), i64_ty.into(), ptr.into()], false);
    get_or_declare(ctx, "kai_array_new", llvm)
}

/// `kai_retain(ptr)` / `kai_release(ptr)` — void over an opaque header.
pub(crate) fn retain_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    get_or_declare(
        ctx,
        "kai_retain",
        ctx.context.void_type().fn_type(&[ptr.into()], false),
    )
}

pub(crate) fn release_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    get_or_declare(
        ctx,
        "kai_release",
        ctx.context.void_type().fn_type(&[ptr.into()], false),
    )
}

/// `kai_string_eq(i8* hdr_a, i8* hdr_b) -> i8` (0/1). Opaque pointer
/// parameters keep heap values uniform (`i8*`) across the whole pipeline;
/// only header-internal GEPs use concrete shapes.
pub(crate) fn string_eq_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx
        .context
        .i8_type()
        .fn_type(&[ptr.into(), ptr.into()], false);
    get_or_declare(ctx, "kai_string_eq", llvm)
}

/// `kai_panic(msg*, msg_len, file*, line, col)` — reports §10.1 and exits;
/// call sites follow with `unreachable`.
#[allow(dead_code)] // referenced once the §10 checks emit call sites
pub(crate) fn panic_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let i64_ty = ctx.context.i64_type();
    let llvm = ctx.context.void_type().fn_type(
        &[
            ptr.into(),
            i64_ty.into(),
            ptr.into(),
            i64_ty.into(),
            i64_ty.into(),
        ],
        false,
    );
    get_or_declare(ctx, "kai_panic", llvm)
}

/// (LLVM symbol, host address) pairs wired into the JIT via global mapping.
/// Taking these addresses also keeps the functions alive in the linked
/// binary; the linker may otherwise strip unreferenced `#[no_mangle]` fns.
pub(crate) const INTRINSICS: [(&str, *const ()); 6] = [
    ("kai_string_new", kai_string_new as *const ()),
    ("kai_array_new", kai_array_new as *const ()),
    ("kai_string_eq", kai_string_eq as *const ()),
    ("kai_retain", kai_retain as *const ()),
    ("kai_release", kai_release as *const ()),
    ("kai_panic", kai_panic as *const ()),
];
