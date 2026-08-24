//! Expression emission. Arithmetic/comparison dispatch on the static Kai
//! type; `&&`/`||` lower to short-circuit control flow with phi merges.

use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::basic_block::BasicBlock;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, FloatValue, IntValue, ValueKind};
use kai_tast::{BinaryOp, KaiType, TypedExpr, TypedExprKind};

/// Runtime intrinsics always return values; normalize the call result.
fn call_value<'ctx>(
    ctx: &Ctx<'ctx>,
    site: inkwell::values::CallSiteValue<'ctx>,
) -> BasicValueEnum<'ctx> {
    match site.try_as_basic_value() {
        ValueKind::Basic(value) => value,
        _ => ctx.context.i32_type().get_undef().into(),
    }
}

pub(crate) fn emit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    expr: &TypedExpr,
) -> BasicValueEnum<'ctx> {
    let ty = expr.ty.clone();
    match &expr.kind {
        TypedExprKind::IntLit(value) => int_const(ctx, *value, &ty).into(),
        TypedExprKind::FloatLit(value) => ctx.context.f64_type().const_float(*value).into(),
        TypedExprKind::BoolLit(value) => ctx
            .context
            .bool_type()
            .const_int(*value as u64, false)
            .into(),
        TypedExprKind::LocalRef(local) => load_local(ctx, frame, *local, &ty),
        TypedExprKind::Neg(inner) => neg(ctx, frame, inner, &ty, expr.span),
        TypedExprKind::Not(inner) => not(ctx, frame, inner),
        TypedExprKind::Binary { op, lhs, rhs } => {
            binary(ctx, frame, *op, lhs, rhs, expr.span)
        }
        // Poisoned recovery node; only reachable in programs that failed
        // upstream. `undef` keeps emission total without inventing behavior.
        TypedExprKind::Invalid => undef_of(ctx, &ty),
        // -- v0.0.6 (§9.9a/§9.10) ----------------------------------------
        TypedExprKind::NoneLit => tagged_none_const(ctx, &ty),
        TypedExprKind::SomeLit(value) => {
            let payload = emit(ctx, frame, value);
            let agg = crate::types::to_llvm(ctx, &ty).into_struct_type().get_undef();
            let with_tag = ctx
                .builder
                .build_insert_value(agg, i64_const(ctx, 0), 0, "some.tag")
                .expect("insert tag");
            ctx.builder
                .build_insert_value(with_tag, payload, 1, "some.payload")
                .expect("insert payload")
                .into_struct_value()
                .into()
        }
        // `lhs ?? rhs` — the rhs evaluates ONLY when lhs is inactive (§9.9a
        // laziness). The result flows through an entry slot so both branches
        // join without a phi; ownership follows the active branch (the pass
        // treats the result as borrowed — see the ownership commit).
        TypedExprKind::Coalesce { lhs, rhs } => {
            lazy_select(ctx, frame, lhs, rhs, &ty)
        }
        TypedExprKind::UnwrapOr { receiver, default } => {
            lazy_select(ctx, frame, receiver, default, &ty)
        }
        // `base catch |err| { stmts.. tail }` (§3.4): the Ok path forwards
        // the payload; the Err path binds the error, runs the block, then
        // evaluates the tail — releases run AFTER the tail (it may read the
        // locals being released).
        TypedExprKind::Catch { base, err_binding, err_ty, stmts, tail, releases } => {
            let recv = emit(ctx, frame, base).into_struct_value();
            let tag = ctx
                .builder
                .build_extract_value(recv, 0, "tag")
                .expect("tag")
                .into_int_value();
            let is_ok = ctx
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, tag, i64_const(ctx, 0), "is.ok")
                .expect("tag cmp");

            let result_llvm = crate::types::to_llvm(ctx, &ty);
            let slot = crate::emit::alloca_in_entry(
                ctx,
                crate::emit::current_function(ctx),
                result_llvm,
                "catch.r",
            );
            let err_slot = crate::emit::alloca_in_entry(
                ctx,
                crate::emit::current_function(ctx),
                to_llvm(ctx, err_ty),
                "catch.err",
            );
            frame.bind(*err_binding, err_slot);

            let fn_v = crate::emit::current_function(ctx);
            let ok_bb = ctx.context.append_basic_block(fn_v, "catch.ok");
            let err_bb = ctx.context.append_basic_block(fn_v, "catch.err");
            let join_bb = ctx.context.append_basic_block(fn_v, "catch.join");
            let _ = ctx.builder.build_conditional_branch(is_ok, ok_bb, err_bb);

            ctx.builder.position_at_end(ok_bb);
            let payload = ctx
                .builder
                .build_extract_value(recv, 1, "ok.payload")
                .expect("ok payload");
            let _ = ctx.builder.build_store(slot, payload);
            let _ = ctx.builder.build_unconditional_branch(join_bb);

            ctx.builder.position_at_end(err_bb);
            let err_val = ctx
                .builder
                .build_extract_value(recv, 2, "err.payload")
                .expect("err payload");
            let _ = ctx.builder.build_store(err_slot, err_val);
            for st in stmts.iter() {
                crate::emit::stmt::emit(ctx, frame, st);
                if terminated_here(ctx) {
                    break;
                }
            }
            if !terminated_here(ctx) {
                let tail_v = emit(ctx, frame, tail);
                let _ = ctx.builder.build_store(slot, tail_v);
                for (local, rty) in releases.iter() {
                    crate::emit::ownership::emit_release_slot(ctx, rty, frame.slot(*local));
                }
                let _ = ctx.builder.build_unconditional_branch(join_bb);
            }

            if !terminated_here(ctx) {
                ctx.builder.position_at_end(join_bb);
            }
            // Value only meaningful when control actually reaches here.
            ctx.builder
                .build_load(result_llvm, slot, "catch.v")
                .expect("load catch result")
        }
        // `f(args)` through a closure VALUE `{ code, env }` (§9.10): env
        // passes as the hidden first parameter; the signature is rebuilt
        // from the static argument/result types.
        TypedExprKind::CallIndirect { callee, args } => {
            let fat = emit(ctx, frame, callee).into_struct_value();
            let code = ctx
                .builder
                .build_extract_value(fat, 0, "clo.code")
                .expect("code ptr")
                .into_pointer_value();
            let env = ctx
                .builder
                .build_extract_value(fat, 1, "clo.env")
                .expect("env ptr");

            let mut arg_vals: Vec<BasicValueEnum<'ctx>> =
                vec![env];
            for a in args {
                arg_vals.push(emit(ctx, frame, a));
            }
            let mut param_tys: Vec<BasicTypeEnum<'ctx>> =
                vec![ctx.context.ptr_type(Default::default()).into()];
            for v in &arg_vals[1..] {
                param_tys.push(v.get_type());
            }
            // fn_type takes metadata enums; map the plain types over.
            let param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                param_tys.iter().map(|t| (*t).into()).collect();

            let llvm = match &ty {
                KaiType::Unit => ctx.context.void_type().fn_type(&param_tys, false),
                r => to_llvm(ctx, r).fn_type(&param_tys, false),
            };
            let args_meta: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = arg_vals
                .iter()
                .map(|v| (*v).into())
                .collect();
            let site = ctx
                .builder
                .build_indirect_call(llvm, code, &args_meta, "icall")
                .expect("indirect call");
            match site.try_as_basic_value() {
                ValueKind::Basic(v) => v,
                _ => ctx.context.i32_type().get_undef().into(),
            }
        }
        // Closure literal (§9.10): the environment is a heap header whose
        // payload carries [captures..., code]; a generated dtor releases
        // heap-bearing captures exactly once at rc==0. The value is the
        // `{ code, env }` fat pointer; the body function takes
        // `(params.., env)` with capture ids bound INTO the payload.
        TypedExprKind::ClosureLit(clo) => {
            let seq = ctx.closure_seq.get();
            ctx.closure_seq.set(seq + 1);
            let fn_name = format!("kai.clo.{seq}");

            let param_llvm: Vec<BasicTypeEnum<'ctx>> = match &ty {
                KaiType::Closure { params, .. } => {
                    params.iter().map(|p| to_llvm(ctx, p)).collect()
                }
                other => unreachable!("closure literal typed {other:?}"),
            };

            let mut field_tys: Vec<BasicTypeEnum<'ctx>> = clo
                .captures
                .iter()
                .map(|c| to_llvm(ctx, &c.ty))
                .collect();
            field_tys.push(ctx.context.ptr_type(Default::default()).into());
            let caps_ty = {
                let name = format!("KaiEnvCaps.{seq}");
                if let Some(existing) = ctx.module.get_struct_type(&name) {
                    existing
                } else {
                    let t = ctx.context.opaque_struct_type(&name);
                    t.set_body(&field_tys, false);
                    t
                }
            };
            let caps_size = caps_ty.size_of().expect("sized captures");

            let any_heap = clo
                .captures
                .iter()
                .any(|c| crate::emit::ownership::heap_bearing(ctx, &c.ty));
            let dtor_val: BasicValueEnum<'ctx> = if any_heap {
                let dname = format!("kai.dtor.env.{seq}");
                let ptr = ctx.context.ptr_type(Default::default());
                let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
                let dtor = ctx.module.add_function(&dname, llvm, None);
                let saved_block = ctx.builder.get_insert_block();
                let entry = ctx.context.append_basic_block(dtor, "entry");
                ctx.builder.position_at_end(entry);
                let hdr = dtor.get_nth_param(0).expect("hdr").into_pointer_value();
                let header_ty = crate::runtime::array_header_ty(ctx, &caps_ty.to_string());
                let payload_ptr_slot = ctx
                    .builder
                    .build_struct_gep(header_ty, hdr, 3, "dtor.payload.p")
                    .expect("payload gep");
                let payload = ctx
                    .builder
                    .build_load(
                        ctx.context.ptr_type(Default::default()),
                        payload_ptr_slot,
                        "dtor.payload",
                    )
                    .expect("payload load")
                    .into_pointer_value();
                for (idx, cap) in clo.captures.iter().enumerate() {
                    if !crate::emit::ownership::heap_bearing(ctx, &cap.ty) {
                        continue;
                    }
                    let slot = ctx
                        .builder
                        .build_struct_gep(caps_ty, payload, idx as u32, "dtor.cap")
                        .expect("capture gep");
                    crate::emit::ownership::emit_release_slot(ctx, &cap.ty, slot);
                }
                let _ = ctx.builder.build_return(None);
                if let Some(saved) = saved_block {
                    ctx.builder.position_at_end(saved);
                }
                dtor.as_global_value().as_pointer_value().into()
            } else {
                ctx.context.ptr_type(Default::default()).const_zero().into()
            };

            // Environment header via the generic runtime allocator.
            let env_hdr = {
                let new_fn = crate::runtime::array_new_fn(ctx);
                let args_meta: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = vec![
                    i64_const(ctx, 1).into(),
                    caps_size.into(),
                    dtor_val.into(),
                ];
                ctx.builder
                    .build_call(new_fn, &args_meta, "env.hdr")
                    .expect("env alloc")
                    .try_as_basic_value()
            };
            let env_hdr = match env_hdr {
                ValueKind::Basic(v) => v.into_pointer_value(),
                _ => unreachable!("array_new returns a pointer"),
            };
            let header_ty = crate::runtime::array_header_ty(ctx, &caps_ty.to_string());
            let payload_ptr_slot = ctx
                .builder
                .build_struct_gep(header_ty, env_hdr, 3, "env.payload.p")
                .expect("payload gep");
            let payload = ctx
                .builder
                .build_load(
                    ctx.context.ptr_type(Default::default()),
                    payload_ptr_slot,
                    "env.payload",
                )
                .expect("payload load")
                .into_pointer_value();

            // Retain + store each capture into the payload.
            for (idx, cap) in clo.captures.iter().enumerate() {
                let src = frame.slot(cap.local);
                let dst = ctx
                    .builder
                    .build_struct_gep(caps_ty, payload, idx as u32, "cap.dst")
                    .expect("capture gep");
                match &cap.ty {
                    KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                        let v = ctx
                            .builder
                            .build_load(to_llvm(ctx, &cap.ty), src, "cap.v")
                            .expect("load capture");
                        crate::emit::ownership::retain_header(ctx, v);
                        let _ = ctx.builder.build_store(dst, v);
                    }
                    k if matches!(k, KaiType::Struct(_) | KaiType::Optional(_) | KaiType::Result { .. })
                        && crate::emit::ownership::heap_bearing(ctx, k) =>
                    {
                        let copy = if matches!(k, KaiType::Struct(_)) {
                            crate::emit::ownership::retain_struct_copy(ctx, k, src)
                        } else {
                            crate::emit::ownership::retain_tagged_copy(ctx, k, src)
                        };
                        let _ = ctx.builder.build_store(dst, copy);
                    }
                    _ => {
                        let v = ctx
                            .builder
                            .build_load(to_llvm(ctx, &cap.ty), src, "cap.v")
                            .expect("load capture");
                        let _ = ctx.builder.build_store(dst, v);
                    }
                }
            }

            // Body function: private, `(params.., env) -> ret`.
            let ret_ty = match &ty {
                KaiType::Closure { ret, .. } => (**ret).clone(),
                other => unreachable!("closure type {other:?}"),
            };
            // Convention: env is the HIDDEN FIRST parameter, matching the
            // indirect-call site which always leads with it.
            let mut sig: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                vec![ctx.context.ptr_type(Default::default()).into()];
            sig.extend(param_llvm.iter().map(|t| inkwell::types::BasicMetadataTypeEnum::from(*t)));
            let llvm = if matches!(ret_ty, KaiType::Unit) {
                ctx.context.void_type().fn_type(&sig, false)
            } else {
                to_llvm(ctx, &ret_ty).fn_type(&sig, false)
            };
            let body_fn = ctx.module.add_function(&fn_name, llvm, None);

            let saved_block = ctx.builder.get_insert_block();
            let entry = ctx.context.append_basic_block(body_fn, "entry");
            ctx.builder.position_at_end(entry);
            let mut inner = Frame::new(frame.module.clone());
            for (idx, pid) in clo.param_ids.iter().enumerate() {
                let arg = body_fn.get_nth_param((idx + 1) as u32).expect("param");
                let pslot = crate::emit::alloca_in_entry(
                    ctx,
                    body_fn,
                    param_llvm[idx],
                    &format!("p{idx}"),
                );
                let _ = ctx.builder.build_store(pslot, arg);
                inner.bind(*pid, pslot);
            }
            // Captures read straight out of THIS function's environment
            // parameter — never the creator's payload instruction.
            let env_arg = body_fn
                .get_nth_param(0)
                .expect("env param")
                .into_pointer_value();
            let body_header_ty = crate::runtime::array_header_ty(ctx, &caps_ty.to_string());
            let body_payload_ptr = ctx
                .builder
                .build_struct_gep(body_header_ty, env_arg, 3, "env.payload.p")
                .expect("payload gep");
            let body_payload = ctx
                .builder
                .build_load(
                    ctx.context.ptr_type(Default::default()),
                    body_payload_ptr,
                    "env.payload",
                )
                .expect("payload load")
                .into_pointer_value();
            for (idx, cap) in clo.captures.iter().enumerate() {
                let view = ctx
                    .builder
                    .build_struct_gep(caps_ty, body_payload, idx as u32, "cap.view")
                    .expect("capture view");
                inner.bind(cap.local, view);
            }
            for st in &clo.body.stmts {
                crate::emit::stmt::emit(ctx, &mut inner, st);
            }
            crate::emit::fallback_return(ctx, &ret_ty);
            if let Some(saved) = saved_block {
                ctx.builder.position_at_end(saved);
            }

            let fat_t = crate::types::closure_fat_ty(ctx);
            let agg = fat_t.get_undef();
            let code_v = body_fn.as_global_value().as_pointer_value().as_basic_value_enum();
            let with_code = ctx
                .builder
                .build_insert_value(agg, code_v, 0, "clo.c")
                .expect("insert code");
            let with_env = ctx
                .builder
                .build_insert_value(with_code, env_hdr.as_basic_value_enum(), 1, "clo.e")
                .expect("insert env");
            with_env.into_struct_value().into()
        }
        TypedExprKind::Call { func, args } => call(ctx, frame, *func, args),
        TypedExprKind::FieldAccess {
            base,
            struct_id,
            field,
        } => field_read(ctx, frame, base, *struct_id, *field, &ty),
        TypedExprKind::StructLit { struct_id, values } => {
            struct_lit(ctx, frame, *struct_id, values)
        }
        TypedExprKind::StrLit { value } => string_lit(ctx, value),
        TypedExprKind::ArrayLit { elements } => {
            let elem = match &expr.ty {
                KaiType::Array(elem) => elem.as_ref().clone(),
                other => unreachable!("array literal typed {other:?}"),
            };
            array_lit(ctx, frame, elements, &elem)
        }
        TypedExprKind::Index { base, index } => {
            index_read(ctx, frame, base, index, &ty, expr.span)
        }
        // Ownership marker from the ownership pass (§9.5): the inner value
        // is borrowed and entering an owning slot. Headers get one refcount
        // op; heap-bearing structs get per-field retains at their source
        // place, then a bitwise copy flows onward.
        TypedExprKind::Retain(inner) => match &expr.ty {
            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                let value = emit(ctx, frame, inner);
                // Closures retain through their ENV header (§9.10); the code
                // pointer is immutable and rides along.
                let env = if matches!(expr.ty, KaiType::Closure { .. }) {
                    let agg = value.into_struct_value();
                    ctx.builder
                        .build_extract_value(agg, 1, "clo.env")
                        .expect("env member")
                } else {
                    value
                };
                crate::emit::ownership::retain_header(ctx, env);
                value
            }
            KaiType::Struct(_)
            | KaiType::Optional(_)
            | KaiType::Result { .. } => {
                // Prefer the source's storage; computed aggregates (e.g. a
                // `??` result) retain through an entry temporary instead.
                let value = emit(ctx, frame, inner);
                let agg_ty = to_llvm(ctx, &expr.ty);
                let tmp = crate::emit::alloca_in_entry(
                    ctx,
                    crate::emit::current_function(ctx),
                    agg_ty,
                    "retain.tmp",
                );
                let _ = ctx.builder.build_store(tmp, value);
                if matches!(expr.ty, KaiType::Struct(_)) {
                    crate::emit::ownership::retain_struct_copy(ctx, &expr.ty, tmp);
                } else {
                    crate::emit::ownership::retain_tagged_copy(ctx, &expr.ty, tmp);
                }
                ctx.builder
                    .build_load(agg_ty, tmp, "retained.v")
                    .expect("load retained")
            }
            other => unreachable!("retain of non-heap type {other:?}"),
        },
    }
}

// -- v0.0.5: heap values ------------------------------------------------------

/// String literal: private global byte blob + `kai_string_new` copy. Every
/// occurrence gets its own allocation for now — interning is a future
/// optimization and changes no observable behavior (§9.7 content equality).
fn string_lit<'ctx>(ctx: &Ctx<'ctx>, value: &str) -> BasicValueEnum<'ctx> {
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
fn array_lit<'ctx>(
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
fn index_read<'ctx>(
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
fn call<'ctx>(
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
fn field_read<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    base: &TypedExpr,
    struct_id: kai_tast::StructId,
    field: u16,
    ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    match place_ptr(ctx, frame, base) {
        Some(base_ptr) => {
            let ptr = super::field_gep(ctx, struct_id, base_ptr, u32::from(field), "field");
            let pointee = crate::types::to_llvm(ctx, ty);
            ctx.builder
                .build_load(pointee, ptr, "field")
                .expect("load from field")
        }
        None => undef_of(ctx, ty), // unreachable post-typecheck
    }
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
            Some(super::field_gep(
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
fn struct_lit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    struct_id: kai_tast::StructId,
    values: &[TypedExpr],
) -> BasicValueEnum<'ctx> {
    let llvm_ty = ctx.structs[struct_id.0 as usize];
    let function = super::current_function(ctx);
    let tmp = super::alloca_in_entry(ctx, function, llvm_ty.into(), "tmp");

    for (idx, value) in values.iter().enumerate() {
        let v = emit(ctx, frame, value);
        let field_ptr = super::field_gep(ctx, struct_id, tmp, idx as u32, "f");
        let _ = ctx.builder.build_store(field_ptr, v);
    }

    let pointee = crate::types::to_llvm(ctx, &KaiType::Struct(struct_id));
    ctx.builder
        .build_load(pointee, tmp, "lit")
        .expect("load literal")
}

fn undef_of<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType) -> BasicValueEnum<'ctx> {
    match crate::types::to_llvm(ctx, ty) {
        inkwell::types::BasicTypeEnum::IntType(int_ty) => int_ty.get_undef().into(),
        inkwell::types::BasicTypeEnum::FloatType(float_ty) => float_ty.get_undef().into(),
        _ => unreachable!("scalar types only"),
    }
}

fn int_const<'ctx>(ctx: &Ctx<'ctx>, value: i64, ty: &KaiType) -> IntValue<'ctx> {
    let int_ty = match *ty {
        KaiType::Int64 => ctx.context.i64_type(),
        _ => ctx.context.i32_type(),
    };
    // `true` = signed interpretation of the two's-complement pattern.
    int_ty.const_int(value as u64, true)
}

fn load_local<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    local: kai_tast::LocalId,
    ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let slot = frame.slot(local);
    let pointee = crate::types::to_llvm(ctx, ty);
    ctx.builder
        .build_load(pointee, slot, "tmp")
        .expect("load from alloca")
}

fn neg<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    operand: &TypedExpr,
    result_ty: &KaiType,
    span: kai_diagnostics::Span,
) -> BasicValueEnum<'ctx> {
    let value = emit(ctx, frame, operand);
    if *result_ty == KaiType::Float64 {
        let as_float = value.into_float_value();
        ctx.builder
            .build_float_neg(as_float, "neg")
            .expect("fneg")
            .into()
    } else {
        // §10.2: `-INT_MIN` overflows just like any other signed op.
        let as_int = value.into_int_value();
        let zero = int_const(ctx, 0, result_ty);
        checked_arith(
            ctx,
            frame,
            span,
            "llvm.ssub.with.overflow",
            zero,
            as_int,
            "neg",
        )
        .into()
    }
}

fn not<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, operand: &TypedExpr) -> BasicValueEnum<'ctx> {
    let value = emit(ctx, frame, operand).into_int_value();
    let truth = ctx.context.bool_type().const_int(1, false);
    ctx.builder
        .build_xor(value, truth, "not")
        .expect("xor for !")
        .into()
}
/// Re-usable arithmetic/comparison core on scalars; also used by compound
/// assignment. Dispatches on the static operand type. Signed arithmetic
/// traps on overflow / division faults (§10.2); floats keep IEEE semantics.
pub(crate) fn apply_binary<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    op: BinaryOp,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    operand_ty: &KaiType,
    span: kai_diagnostics::Span,
) -> BasicValueEnum<'ctx> {
    match operand_ty {
        KaiType::Float64 => float_arith(ctx, op, lhs.into_float_value(), rhs.into_float_value()),
        // Strings compare by CONTENT through the runtime; pointer identity
        // is never observable (§9.7). Ne is the inverted eq.
        KaiType::String if matches!(op, BinaryOp::Eq | BinaryOp::Ne) => {
            let raw = call_value(
                ctx,
                ctx.builder
                    .build_call(
                        crate::runtime::string_eq_fn(ctx),
                        &[
                            lhs.into_pointer_value().into(),
                            rhs.into_pointer_value().into(),
                        ],
                        "str.eq",
                    )
                    .expect("kai_string_eq call"),
            )
            .into_int_value();
            let truth = ctx.builder.build_int_truncate_or_bit_cast(
                raw,
                ctx.context.bool_type(),
                "str.eq.b",
            ).expect("trunc to i1");
            if op == BinaryOp::Ne {
                let one = ctx.context.bool_type().const_int(1, false);
                ctx.builder.build_xor(truth, one, "str.ne").expect("xor")
            } else {
                truth
            }
            .into()
        }
        _ => int_arith(ctx, frame, op, lhs.into_int_value(), rhs.into_int_value(), span).into(),
    }
}

/// `{iN result, i1 flag} @llvm.s<op>.with.overflow.iN(iN, iN)` declared on
/// demand; LLVM recognizes these by exact name.
fn overflow_intrinsic<'ctx>(
    ctx: &Ctx<'ctx>,
    name: &str,
    int_ty: inkwell::types::IntType<'ctx>,
) -> inkwell::values::FunctionValue<'ctx> {
    if let Some(existing) = ctx.module.get_function(name) {
        return existing;
    }
    let pair = ctx
        .context
        .struct_type(
            &[int_ty.into(), ctx.context.bool_type().into()],
            false,
        );
    ctx.module
        .add_function(name, pair.fn_type(&[int_ty.into(), int_ty.into()], false), None)
}

/// Emits `llvm.s<op>.with.overflow` for `lhs`'s width, traps on the
/// overflow flag (§10.2), and yields the arithmetic result.
fn checked_arith<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    span: kai_diagnostics::Span,
    intrinsic: &str,
    lhs: IntValue<'ctx>,
    rhs: IntValue<'ctx>,
    name: &str,
) -> IntValue<'ctx> {
    let pair_fn = {
        // LLVM requires the width-suffixed exact name (`...i32`, `...i64`).
        let mangled = format!("{intrinsic}.i{}", lhs.get_type().get_bit_width());
        overflow_intrinsic(ctx, &mangled, lhs.get_type())
    };
    let call = ctx
        .builder
        .build_call(pair_fn, &[lhs.into(), rhs.into()], "ovf")
        .expect("checked arith call");
    let res = match call.try_as_basic_value() {
        ValueKind::Basic(value) => value.into_struct_value(),
        _ => unreachable!("overflow intrinsic returns a struct"),
    };
    let flag = ctx
        .builder
        .build_extract_value(res, 1, "ovf.flag")
        .expect("flag slot")
        .into_int_value();
    crate::emit::panic::trap_on(ctx, frame, span, flag, "integer overflow", "arith.ok");
    ctx.builder
        .build_extract_value(res, 0, name)
        .expect(name)
        .into_int_value()
}

/// §10.2: division faults — a zero divisor panics with its own message and
/// `MIN / -1` (the one quotient outside the type's range) panics as an
/// integer overflow.
fn div_guard<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    span: kai_diagnostics::Span,
    lhs: IntValue<'ctx>,
    rhs: IntValue<'ctx>,
    zero_message: &str,
    cont_label: &str,
) {
    let b = &ctx.builder;
    let zero = rhs.get_type().const_zero();
    let is_zero = b
        .build_int_compare(inkwell::IntPredicate::EQ, rhs, zero, "rhs.zero")
        .expect("zero divisor icmp");
    crate::emit::panic::trap_on(ctx, frame, span, is_zero, zero_message, cont_label);

    let minus_one = rhs.get_type().const_int((-1i64) as u64, true);
    let rhs_is_m1 = b
        .build_int_compare(inkwell::IntPredicate::EQ, rhs, minus_one, "rhs.m1")
        .expect("-1 icmp");
    // The only overflowing quotient is MIN / -1.
    let min = if lhs.get_type().get_bit_width() == 64 {
        i64::MIN
    } else {
        i32::MIN as i64
    };
    let lhs_is_min = b
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            lhs,
            lhs.get_type().const_int(min as u64, true),
            "lhs.min",
        )
        .expect("MIN icmp");
    let ovf = b
        .build_and(rhs_is_m1, lhs_is_min, "min.div")
        .expect("conjunction");
    crate::emit::panic::trap_on(ctx, frame, span, ovf, "integer overflow", "safe.div");
}

fn int_arith<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    op: BinaryOp,
    lhs: IntValue<'ctx>,
    rhs: IntValue<'ctx>,
    span: kai_diagnostics::Span,
) -> IntValue<'ctx> {
    let b = &ctx.builder;
    match op {
        BinaryOp::Add => checked_arith(
            ctx,
            frame,
            span,
            "llvm.sadd.with.overflow",
            lhs,
            rhs,
            "add",
        ),
        BinaryOp::Sub => checked_arith(
            ctx,
            frame,
            span,
            "llvm.ssub.with.overflow",
            lhs,
            rhs,
            "sub",
        ),
        BinaryOp::Mul => checked_arith(
            ctx,
            frame,
            span,
            "llvm.smul.with.overflow",
            lhs,
            rhs,
            "mul",
        ),
        BinaryOp::Div => {
            div_guard(ctx, frame, span, lhs, rhs, "division by zero", "div.safe");
            b.build_int_signed_div(lhs, rhs, "div").expect("sdiv")
        }
        BinaryOp::Mod => {
            div_guard(ctx, frame, span, lhs, rhs, "modulo by zero", "rem.safe");
            b.build_int_signed_rem(lhs, rhs, "rem").expect("srem")
        }
        BinaryOp::Lt => b
            .build_int_compare(inkwell::IntPredicate::SLT, lhs, rhs, "lt")
            .expect("icmp"),
        BinaryOp::Gt => b
            .build_int_compare(inkwell::IntPredicate::SGT, lhs, rhs, "gt")
            .expect("icmp"),
        BinaryOp::Le => b
            .build_int_compare(inkwell::IntPredicate::SLE, lhs, rhs, "le")
            .expect("icmp"),
        BinaryOp::Ge => b
            .build_int_compare(inkwell::IntPredicate::SGE, lhs, rhs, "ge")
            .expect("icmp"),
        // Equality works on ints and bools alike (i1).
        BinaryOp::Eq => b
            .build_int_compare(inkwell::IntPredicate::EQ, lhs, rhs, "eq")
            .expect("icmp"),
        BinaryOp::Ne => b
            .build_int_compare(inkwell::IntPredicate::NE, lhs, rhs, "ne")
            .expect("icmp"),
        BinaryOp::And | BinaryOp::Or => {
            unreachable!("short-circuit ops never reach scalar emission")
        }
    }
}

fn float_arith<'ctx>(
    ctx: &Ctx<'ctx>,
    op: BinaryOp,
    lhs: FloatValue<'ctx>,
    rhs: FloatValue<'ctx>,
) -> BasicValueEnum<'ctx> {
    let b = &ctx.builder;
    match op {
        BinaryOp::Add => b.build_float_add(lhs, rhs, "fadd").expect("fadd").into(),
        BinaryOp::Sub => b.build_float_sub(lhs, rhs, "fsub").expect("fsub").into(),
        BinaryOp::Mul => b.build_float_mul(lhs, rhs, "fmul").expect("fmul").into(),
        BinaryOp::Div => b.build_float_div(lhs, rhs, "fdiv").expect("fdiv").into(),
        BinaryOp::Lt => b
            .build_float_compare(inkwell::FloatPredicate::OLT, lhs, rhs, "flt")
            .expect("fcmp")
            .into(),
        BinaryOp::Gt => b
            .build_float_compare(inkwell::FloatPredicate::OGT, lhs, rhs, "fgt")
            .expect("fcmp")
            .into(),
        BinaryOp::Le => b
            .build_float_compare(inkwell::FloatPredicate::OLE, lhs, rhs, "fle")
            .expect("fcmp")
            .into(),
        BinaryOp::Ge => b
            .build_float_compare(inkwell::FloatPredicate::OGE, lhs, rhs, "fge")
            .expect("fcmp")
            .into(),
        BinaryOp::Eq => b
            .build_float_compare(inkwell::FloatPredicate::OEQ, lhs, rhs, "feq")
            .expect("fcmp")
            .into(),
        BinaryOp::Ne => b
            .build_float_compare(inkwell::FloatPredicate::UNE, lhs, rhs, "fne")
            .expect("fcmp")
            .into(),
        BinaryOp::Mod | BinaryOp::And | BinaryOp::Or => {
            unreachable!("rejected upstream by the type checker")
        }
    }
}

fn binary<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    op: BinaryOp,
    lhs: &TypedExpr,
    rhs: &TypedExpr,
    span: kai_diagnostics::Span,
) -> BasicValueEnum<'ctx> {
    match op {
        // `a && b`: evaluate b only when a is true.
        // `a || b`: evaluate b only when a is false.
        BinaryOp::And => short_circuit(ctx, frame, lhs, rhs, false),
        BinaryOp::Or => short_circuit(ctx, frame, lhs, rhs, true),
        _ => {
            let l = emit(ctx, frame, lhs);
            let r = emit(ctx, frame, rhs);
            apply_binary(ctx, frame, op, l, r, &lhs.ty, span)
        }
    }
}

/// Lowers `lhs OP rhs` for OP in {&&, ||} using explicit branches plus a phi;
/// the rhs side effect (e.g. an assignment-heavy expression later) must not
/// run when short-circuited.
fn short_circuit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    lhs_expr: &TypedExpr,
    rhs_expr: &TypedExpr,
    is_or: bool,
) -> BasicValueEnum<'ctx> {
    let name = if is_or { "or" } else { "and" };
    let lhs_val = emit(ctx, frame, lhs_expr).into_int_value();

    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    let function = current.get_parent().expect("block belongs to a function");
    let rhs_block = ctx
        .context
        .append_basic_block(function, &format!("{name}.rhs"));
    let merge_block = ctx
        .context
        .append_basic_block(function, &format!("{name}.end"));

    // For `&&`: rhs runs when lhs holds; for `||`: rhs runs when lhs fails.
    let (true_target, false_target) = if is_or {
        (merge_block, rhs_block)
    } else {
        (rhs_block, merge_block)
    };
    let _ = ctx
        .builder
        .build_conditional_branch(lhs_val, true_target, false_target);

    ctx.builder.position_at_end(rhs_block);
    let rhs_val = emit(ctx, frame, rhs_expr).into_int_value();
    let rhs_end: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    if rhs_end.get_terminator().is_none() {
        let _ = ctx.builder.build_unconditional_branch(merge_block);
    }

    ctx.builder.position_at_end(merge_block);
    let phi = ctx
        .builder
        .build_phi(ctx.context.bool_type(), &format!("{name}.result"))
        .expect("phi node");
    phi.add_incoming(&[(&lhs_val, current), (&rhs_val, rhs_end)]);
    phi.as_basic_value()
}

// -- v0.0.6 helpers (§9.9a) -----------------------------------------------------

fn i64_const<'ctx>(ctx: &Ctx<'ctx>, v: u64) -> inkwell::values::IntValue<'ctx> {
    ctx.context.i64_type().const_int(v, false)
}

/// `{ tag = 1 }` with a zeroed payload — the None/absent shape. Payload
/// fields are zeroed per LLVM kind; nested aggregates stay undef (they are
/// never read while the tag says absent).
fn tagged_none_const<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType) -> BasicValueEnum<'ctx> {
    let llvm = crate::types::to_llvm(ctx, ty).into_struct_type();
    let mut fields: Vec<BasicValueEnum<'ctx>> = vec![i64_const(ctx, 1).into()];
    for idx in 1..llvm.count_fields() {
        fields.push(zero_of(ctx, llvm.get_field_type_at_index(idx).expect("field")));
    }
    llvm.const_named_struct(&fields).into()
}

fn zero_of<'ctx>(ctx: &Ctx<'ctx>, ty: inkwell::types::BasicTypeEnum<'ctx>) -> BasicValueEnum<'ctx> {
    match ty {
        inkwell::types::BasicTypeEnum::PointerType(_) => {
            ctx.context.ptr_type(Default::default()).const_zero().into()
        }
        inkwell::types::BasicTypeEnum::IntType(t) => t.const_zero().into(),
        inkwell::types::BasicTypeEnum::FloatType(t) => t.const_zero().into(),
        other => other.into_struct_type().get_undef().into(),
    }
}

/// Shared `??` / `.unwrap_or` shape: both are "active branch's payload or
/// the fallback". Tag 0 = Some/Ok in either layout; payload sits at member 1.
fn lazy_select<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    receiver: &TypedExpr,
    fallback: &TypedExpr,
    result_ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let recv = emit(ctx, frame, receiver).into_struct_value();
    let tag = ctx
        .builder
        .build_extract_value(recv, 0, "tag")
        .expect("tag")
        .into_int_value();
    let active = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, tag, i64_const(ctx, 0), "active")
        .expect("tag cmp");

    let result_llvm = crate::types::to_llvm(ctx, result_ty);
    let slot = crate::emit::alloca_in_entry(
        ctx,
        crate::emit::current_function(ctx),
        result_llvm,
        "co.r",
    );
    let some_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.some");
    let else_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.fallback");
    let join_bb = ctx
        .context
        .append_basic_block(crate::emit::current_function(ctx), "co.join");
    let _ = ctx
        .builder
        .build_conditional_branch(active, some_bb, else_bb);

    // Active: forward the payload bits untouched — the consumer's retain
    // (borrow semantics) balances against its later release.
    ctx.builder.position_at_end(some_bb);
    let payload = ctx
        .builder
        .build_extract_value(recv, 1, "payload")
        .expect("payload");
    let _ = ctx.builder.build_store(slot, payload);
    let _ = ctx.builder.build_unconditional_branch(join_bb);

    // Fallback: evaluate lazily; when it produced an OWNED header, drop the
    // creator's reference after copying into the slot (§9.9a scheme).
    ctx.builder.position_at_end(else_bb);
    let d = emit(ctx, frame, fallback);
    let _ = ctx.builder.build_store(slot, d);
    if crate::emit::ownership::heap_bearing(ctx, result_ty) {
        match result_ty {
            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                crate::emit::ownership::release_header_value(ctx, d);
            }
            other => {
                let tmp = crate::emit::alloca_in_entry(
                    ctx,
                    crate::emit::current_function(ctx),
                    result_llvm,
                    "co.tmp",
                );
                let _ = ctx.builder.build_store(tmp, d);
                crate::emit::ownership::emit_release_slot(ctx, other, tmp);
            }
        }
    }
    let _ = ctx.builder.build_unconditional_branch(join_bb);

    ctx.builder.position_at_end(join_bb);
    ctx.builder
        .build_load(result_llvm, slot, "co.v")
        .expect("load coalesced")
}


/// True when the current insert block already has a terminator (an early
/// `return` inside a catch body leaves it that way).
fn terminated_here(ctx: &Ctx<'_>) -> bool {
    ctx.builder
        .get_insert_block()
        .and_then(|b| b.get_terminator())
        .is_some()
}
