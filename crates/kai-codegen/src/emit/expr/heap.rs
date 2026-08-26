#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Heap emission — strings, arrays, places, struct literals.
use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, IntValue};
use kai_tast::{KaiType, TypedExpr, TypedExprKind};

use super::{call_value, emit, int_const, load_local, undef_of};
use crate::emit::{alloca_in_entry, current_function, field_gep};

pub(crate) fn string_lit<'ctx>(ctx: &Ctx<'ctx>, value: &str) -> BasicValueEnum<'ctx> {
    let bytes = value.as_bytes();
    let blob_ty = ctx.context.i8_type().array_type(bytes.len() as u32);
    let global = ctx
        .module
        .add_global(blob_ty, Some(inkwell::AddressSpace::default()), "kai.str");
    global.set_initializer(&ctx.context.const_string(bytes, false));
    global.set_constant(true);
    global.set_unnamed_addr(true);
    global.set_linkage(inkwell::module::Linkage::Private);

    let zero = ctx.context.i32_type().const_zero();
    let data_ptr = unsafe {
        ctx.builder
            .build_in_bounds_gep(
                blob_ty,
                global.as_pointer_value(),
                &[zero, zero],
                "str.data",
            )
            .expect("gep to string blob")
    };
    let new_fn = crate::runtime::string_new_fn(ctx);
    let len = ctx.context.i64_type().const_int(bytes.len() as u64, false);
    let site = ctx
        .builder
        .build_call(new_fn, &[data_ptr.into(), len.into()], "str")
        .expect("kai_string_new call");
    call_value(ctx, site)
}

/// Array literal: allocate the header, then evaluate + store each element.
/// Elements are evaluated left-to-right AFTER allocation, so element exprs
/// may themselves build arrays or strings freely.

pub(crate) fn array_lit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    elements: &[TypedExpr],
    elem_ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let elem_llvm = crate::types::to_llvm(ctx, elem_ty);
    let _header_ty = crate::runtime::array_header_ty(ctx, &elem_llvm.to_string());

    let new_fn = crate::runtime::array_new_fn(ctx);
    let len = ctx
        .context
        .i64_type()
        .const_int(elements.len() as u64, false);
    // LLVM resolves this sizeof constant expression against the target
    // layout; no host-side layout math needed.
    let elem_size_v = elem_llvm
        .size_of()
        .expect("array elements are sized types");
    // Arrays own their elements (§9.9): when the last owner releases the
    // header, this destructor releases every element exactly once.
    let null_ptr = ctx.context.ptr_type(Default::default()).const_zero();
    let dtor = crate::emit::ownership::ensure_elem_dtor(ctx, elem_ty)
        .map_or(null_ptr.into(), |f| {
            f.as_global_value().as_pointer_value().into()
        });
    let header = call_value(
        ctx,
        ctx.builder
            .build_call(new_fn, &[len.into(), elem_size_v.into(), dtor], "arr")
            .expect("kai_array_new call"),
    )
    .into_pointer_value();

    let elems_slot = elems_storage_of(ctx, header, elem_llvm);
    for (idx, element) in elements.iter().enumerate() {
        let value = emit(ctx, frame, element);
        let i = ctx.context.i64_type().const_int(idx as u64, false);
        let slot = unsafe {
            ctx.builder
                .build_in_bounds_gep(elem_llvm, elems_slot, &[i], "arr.slot")
                .expect("element gep")
        };
        let _ = ctx.builder.build_store(slot, value);
    }

    header.into()
}

/// Indices are any integer width at the language level; GEPs want i64.

pub(crate) fn widen_index<'ctx>(
    ctx: &Ctx<'ctx>,
    idx: inkwell::values::IntValue<'ctx>,
) -> inkwell::values::IntValue<'ctx> {
    if idx.get_type().get_bit_width() == 64 {
        return idx;
    }
    // Signed: negative indices are UB territory anyway; the cast preserves
    // whatever bit pattern arrives so downstream behavior stays consistent.
    ctx.builder
        .build_int_cast_sign_flag(idx, ctx.context.i64_type(), true, "idx64")
        .expect("index cast")
}

/// Loads the `elems` storage pointer out of an OPAQUE header pointer,
/// transiently casting to the concrete `%KaiArray.<elem>` shape. Heap values
/// themselves always travel as `i8*` so storage stays layout-uniform.

pub(crate) fn elems_storage_of<'ctx>(
    ctx: &Ctx<'ctx>,
    header: inkwell::values::PointerValue<'ctx>,
    elem_ty: BasicTypeEnum<'ctx>,
) -> inkwell::values::PointerValue<'ctx> {
    let header_ty = crate::runtime::array_header_ty(ctx, &elem_ty.to_string());
    let typed = ctx
        .builder
        .build_pointer_cast(header, ctx.context.ptr_type(Default::default()), "arr.hdr")
        .expect("hdr cast");
    let elems_ptr_ty = header_ty
        .get_field_type_at_index(3)
        .expect("header has elems field");
    let field_slot = ctx
        .builder
        .build_struct_gep(header_ty, typed, 3, "arr.elems.p")
        .expect("header gep");
    ctx.builder
        .build_load(elems_ptr_ty, field_slot, "arr.elems")
        .expect("load elems")
        .into_pointer_value()
}

/// Header pointer out of an emitted array value; post-typecheck an array
/// expression is always a heap-header pointer.

pub(crate) fn header_of_value<'ctx>(
    value: BasicValueEnum<'ctx>,
) -> inkwell::values::PointerValue<'ctx> {
    match value {
        BasicValueEnum::PointerValue(p) => p,
        _ => unreachable!("array base is always a header pointer"),
    }
}

/// The authoritative element count for bounds checks and `for..in` loop
/// conditions, loaded straight from the header's `len` field.

pub(crate) fn header_len<'ctx>(
    ctx: &Ctx<'ctx>,
    header: inkwell::values::PointerValue<'ctx>,
    elem_ty: BasicTypeEnum<'ctx>,
) -> IntValue<'ctx> {
    let header_ty = crate::runtime::array_header_ty(ctx, &elem_ty.to_string());
    let typed = ctx
        .builder
        .build_pointer_cast(header, ctx.context.ptr_type(Default::default()), "arr.hdr")
        .expect("hdr cast");
    let len_slot = ctx
        .builder
        .build_struct_gep(header_ty, typed, 1, "arr.len.p")
        .expect("len gep");
    let loaded = header_ty.get_field_type_at_index(1).expect("len field");
    ctx.builder
        .build_load(loaded, len_slot, "arr.len")
        .expect("len load")
        .into_int_value()
}

/// §10: every indexed access traps when the index falls outside `0..len`.
/// Emits the compare in the current block and branches through a panic
/// block; emission continues in a fresh continuation block.

pub(crate) fn bounds_check<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    span: kai_diagnostics::Span,
    header: inkwell::values::PointerValue<'ctx>,
    elem_ty: BasicTypeEnum<'ctx>,
    idx64: IntValue<'ctx>,
) {
    let len = header_len(ctx, header, elem_ty);
    let zero = idx64.get_type().const_int(0, false);
    // Signed pair test: `idx >= 0 && idx < len`. Negative indices are as
    // out-of-bounds as past-the-end ones.
    let below = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::SGE, idx64, zero, "bnd.low")
        .expect("lower bound icmp");
    let above = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::SLT, idx64, len, "bnd.high")
        .expect("upper bound icmp");
    let ok = ctx
        .builder
        .build_and(below, above, "bnd.ok")
        .expect("bound conjunction");
    crate::emit::panic::trap_on(
        ctx,
        frame,
        span,
        ctx.builder
            .build_not(ok, "bnd.bad")
            .expect("negate guard"),
        "array index out of bounds",
        "in.bounds",
    );
}

/// Element address for one indexed hop — THE shared core of array reads,
/// rvalue places, and assignment-place steps: bounds guard, elems storage,
/// GEP. `for..in` does not route through here: its induction variable is
/// bounded by the same `len` it reads, so a per-iteration trap is dead
/// weight.

pub(crate) fn elem_slot<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    span: kai_diagnostics::Span,
    header: inkwell::values::PointerValue<'ctx>,
    elem_ty: BasicTypeEnum<'ctx>,
    idx64: IntValue<'ctx>,
    name: &str,
) -> inkwell::values::PointerValue<'ctx> {
    bounds_check(ctx, frame, span, header, elem_ty, idx64);
    let elems = elems_storage_of(ctx, header, elem_ty);
    unsafe {
        ctx.builder
            .build_in_bounds_gep(elem_ty, elems, &[idx64], name)
            .expect("element gep")
    }
}

/// `base[index]` read: load header, bounds-check the index, GEP into
/// elems, load the element. A pure borrow of one slot — ownership never
/// moves (§9.9).

pub(crate) fn index_read<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    base: &TypedExpr,
    index: &TypedExpr,
    result_ty: &KaiType,
    span: kai_diagnostics::Span,
) -> BasicValueEnum<'ctx> {
    let header = header_of_value(emit(ctx, frame, base));

    // An index read yields the element, so the element's llvm type doubles
    // as the header-shape key here.
    let elem_ty = crate::types::to_llvm(ctx, result_ty);
    let idx64 = widen_index(ctx, emit(ctx, frame, index).into_int_value());
    let slot = elem_slot(ctx, frame, span, header, elem_ty, idx64, "elem.slot");
    ctx.builder
        .build_load(elem_ty, slot, "elem")
        .expect("element load")
}

/// Direct call to a declared function. Arguments pass by value (§9.3); unit
/// results have no LLVM value, so callers get an `undef` placeholder they
/// always discard.

pub(crate) fn call<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    func: kai_tast::FunctionId,
    args: &[TypedExpr],
) -> BasicValueEnum<'ctx> {
    let function = ctx.functions[func.0 as usize];
    let arg_values: Vec<BasicValueEnum<'ctx>> =
        args.iter().map(|arg| emit(ctx, frame, arg)).collect();

    let args_meta: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = arg_values
        .into_iter()
        .map(inkwell::values::BasicMetadataValueEnum::from)
        .collect();
    let site = ctx
        .builder
        .build_call(function, &args_meta, "call")
        .expect("direct call");
    match site.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(value) => value,
        _ => ctx.context.i32_type().get_undef().into(),
    }
}

/// Reads a field: GEP the base place by this access's index, then load.
///
/// v0.0.8.1 (BUG-2/3): when the base is NOT a place — an rvalue computed
/// aggregate such as `v.unwrap_or(default)` or a call result — the value is
/// materialized into an entry-block temporary first, then GEP'd. The old
/// behavior emitted `undef` silently here, which made chained field reads on
/// computed aggregates return register garbage (silent data corruption).

pub(crate) fn field_read<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    base: &TypedExpr,
    struct_id: kai_tast::StructId,
    field: u16,
    ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let base_ptr = match place_ptr(ctx, frame, base) {
        Some(ptr) => ptr,
        None => {
            // Rvalue base: emit it and spill to a temporary so the field
            // GEP has real storage to read from.
            let value = emit(ctx, frame, base);
            let agg_ty = crate::types::to_llvm(ctx, &base.ty);
            let tmp = crate::emit::alloca_in_entry(
                ctx,
                crate::emit::current_function(ctx),
                agg_ty,
                "field.base.tmp",
            );
            let _ = ctx.builder.build_store(tmp, value);
            tmp
        }
    };
    let ptr = crate::emit::field_gep(ctx, struct_id, base_ptr, u32::from(field), "field");
    let pointee = crate::types::to_llvm(ctx, ty);
    ctx.builder
        .build_load(pointee, ptr, "field")
        .expect("load from field")
}

/// Address of an lvalue-shaped expression. Struct-typed expressions are
/// exactly the places; anything else has no address.

pub(crate) fn place_ptr<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    expr: &TypedExpr,
) -> Option<inkwell::values::PointerValue<'ctx>> {
    match &expr.kind {
        TypedExprKind::LocalRef(local) => Some(frame.slot(*local)),
        TypedExprKind::FieldAccess {
            base,
            struct_id,
            field,
        } => {
            let base_ptr = place_ptr(ctx, frame, base)?;
            Some(crate::emit::field_gep(
                ctx,
                *struct_id,
                base_ptr,
                u32::from(*field),
                "place",
            ))
        }
        TypedExprKind::Index { base, index } => {
            // The INDEX node's type is the ELEMENT type (§9.9).
            let elem_llvm = crate::types::to_llvm(ctx, &expr.ty);
            let header = header_of_value(emit(ctx, frame, base));
            let idx64 = widen_index(ctx, emit(ctx, frame, index).into_int_value());
            Some(elem_slot(
                ctx,
                frame,
                expr.span,
                header,
                elem_llvm,
                idx64,
                "place.elem",
            ))
        }
        _ => None,
    }
}

/// Materializes `Name { .. }`: an entry-block temporary filled field-by-field
/// (declaration order — the type checker already reordered the values).

pub(crate) fn struct_lit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    struct_id: kai_tast::StructId,
    values: &[TypedExpr],
) -> BasicValueEnum<'ctx> {
    let llvm_ty = ctx.structs[struct_id.0 as usize];
    let function = crate::emit::current_function(ctx);
    let tmp = crate::emit::alloca_in_entry(ctx, function, llvm_ty.into(), "tmp");

    for (idx, value) in values.iter().enumerate() {
        let v = emit(ctx, frame, value);
        let field_ptr = crate::emit::field_gep(ctx, struct_id, tmp, idx as u32, "f");
        let _ = ctx.builder.build_store(field_ptr, v);
    }

    let pointee = crate::types::to_llvm(ctx, &KaiType::Struct(struct_id));
    ctx.builder
        .build_load(pointee, tmp, "lit")
        .expect("load literal")
}

