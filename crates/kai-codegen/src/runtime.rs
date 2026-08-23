//! Host-side runtime for heap values (v0.0.5).
//!
//! Intrinsics are plain Rust symbols exported from this process; MCJIT
//! resolves them by name at link time, so JIT programs call them like any
//! imported function. AOT output (`compile_ir`) carries declarations only.
//!
//! Layouts (§9.1): every heap value is a POINTER to a fixed header. The
//! refcount field is reserved NOW so phase D (ownership) never changes the
//! ABI — until retain/release land it stays 0.
//!
//! ```text
//! KaiString        { i64 rc, i64 len, i8* data }
//! KaiArray.<elem>  { i64 rc, i64 len, <elem>* elems }
//! ```

use crate::context::Ctx;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

/// Header shared by every string value. `data` holds exactly `len` bytes;
/// NUL-termination is NOT guaranteed — lengths are authoritative.
#[repr(C)]
pub struct KaiStringHeader {
    pub rc: i64,
    pub len: i64,
    pub data: *mut u8,
}

/// Header shared by every array value regardless of element type; element
/// size rides at the call site because the header never dereferences elems.
#[repr(C)]
pub struct KaiArrayHeader {
    pub rc: i64,
    pub len: i64,
    /// Untyped storage: callers GEP with the static element type.
    pub elems: *mut u8,
}

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
pub unsafe extern "C" fn kai_string_new(data: *const u8, len: i64) -> *mut KaiStringHeader {
    let byte_len = usize::try_from(len).unwrap_or(0);
    let buf = alloc_bytes(byte_len);
    if byte_len > 0 {
        debug_assert!(!data.is_null());
        unsafe { std::ptr::copy_nonoverlapping(data, buf, byte_len) };
    }
    Box::into_raw(Box::new(KaiStringHeader {
        rc: 0,
        len,
        data: buf,
    }))
}

/// `[..]` -> owned header + zero-initialized element storage. Zero-init so
/// an element slot is never observed uninitialized even on buggy paths.
#[unsafe(no_mangle)]
pub extern "C" fn kai_array_new(len: i64, elem_size: i64) -> *mut KaiArrayHeader {
    let byte_len = usize::try_from(len).unwrap_or(0).saturating_mul(
        usize::try_from(elem_size).unwrap_or(0),
    );
    let elems = alloc_bytes(byte_len);
    if byte_len > 0 {
        // SAFETY: `elems` covers exactly `byte_len` writable bytes.
        unsafe { std::ptr::write_bytes(elems, 0, byte_len) };
    }
    Box::into_raw(Box::new(KaiArrayHeader {
        rc: 0,
        len,
        elems,
    }))
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
pub unsafe extern "C" fn kai_string_eq(a: *const KaiStringHeader, b: *const KaiStringHeader) -> u8 {
    let (a, b) = unsafe { (&*a, &*b) };
    if a.len != b.len {
        return 0;
    }
    let byte_len = usize::try_from(a.len).unwrap_or(0);
    if byte_len == 0 || a.data == b.data {
        return 1;
    }
    // SAFETY: both payloads hold at least `byte_len` readable bytes.
    let same = unsafe {
        std::slice::from_raw_parts(a.data, byte_len) == std::slice::from_raw_parts(b.data, byte_len)
    };
    u8::from(same)
}

// -- LLVM-side plumbing -------------------------------------------------------

/// `%KaiString` named struct type, created once per module. Unused while
/// strings travel opaque; the ownership phase (retain/release, data access)
/// GEPs through this shape.
#[allow(dead_code)]
pub(crate) fn string_header_ty<'ctx>(ctx: &Ctx<'ctx>) -> StructType<'ctx> {
    if let Some(existing) = ctx.module.get_struct_type("KaiString") {
        return existing;
    }
    let ty = ctx.context.opaque_struct_type("KaiString");
    let i64_ty = ctx.context.i64_type().into();
    let data_ptr = ctx.context.ptr_type(Default::default()).into();
    ty.set_body(&[i64_ty, i64_ty, data_ptr], false);
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
    let i64_ty = ctx.context.i64_type().into();
    // Opaque pointer era: every elems field is `ptr`, whatever T is.
    let elems_ptr = ctx.context.ptr_type(Default::default()).into();
    ty.set_body(&[i64_ty, i64_ty, elems_ptr], false);
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

/// `kai_array_new(i64 len, i64 elem_size) -> %KaiArray.<elem>*`
pub(crate) fn array_new_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let i64_ty = ctx.context.i64_type();
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ptr.fn_type(&[i64_ty.into(), i64_ty.into()], false);
    get_or_declare(ctx, "kai_array_new", llvm)
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

/// (LLVM symbol, host address) pairs wired into the JIT via global mapping.
/// Taking these addresses also keeps the functions alive in the linked
/// binary; the linker may otherwise strip unreferenced `#[no_mangle]` fns.
pub(crate) const INTRINSICS: [(&str, *const ()); 3] = [
    ("kai_string_new", kai_string_new as *const ()),
    ("kai_array_new", kai_array_new as *const ()),
    ("kai_string_eq", kai_string_eq as *const ()),
];
