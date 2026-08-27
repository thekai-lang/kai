#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Expression emission. Arithmetic/comparison dispatch on the static Kai
//! type; `&&`/`||` lower to short-circuit control flow with phi merges.

use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, IntValue, ValueKind};
use kai_tast::{KaiType, TypedExpr, TypedExprKind};

/// Runtime intrinsics always return values; normalize the call result.
mod arith;
mod heap;
mod tagged;
mod closure;
pub(crate) use heap::{array_lit, bounds_check, call, elem_slot, elems_storage_of, field_read, header_len, header_of_value, place_ptr, string_lit, struct_lit, widen_index};
pub(crate) use tagged::{i64_const, lazy_select, tagged_none_const, terminated_here, zero_of};
pub(crate) use closure::emit_closure;

pub(crate) use arith::apply_binary;
pub(crate) fn call_value<'ctx>(
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
        TypedExprKind::IntLit(value) => {
            // Scalar temporal literal (§5.1.7): same header-wrap as string
            // literals — bare i32/i64 here made every later release treat
            // the scalar as a heap pointer.
            if let KaiType::Temporal {
                inner,
                origin: kai_tast::TemporalOrigin::Wallclock,
                ..
            } = &expr.ty
            {
                let base = int_const(ctx, *value, inner).into();
                crate::emit::wallclock::wallclock_construct(ctx, inner, base)
            } else {
                int_const(ctx, *value, &ty).into()
            }
        }
        TypedExprKind::FloatLit(value) => ctx.context.f64_type().const_float(*value).into(),
        TypedExprKind::BoolLit(value) => ctx
            .context
            .bool_type()
            .const_int(*value as u64, false)
            .into(),
        TypedExprKind::LocalRef(local) => load_local(ctx, frame, *local, &ty),
        TypedExprKind::Neg(inner) => arith::neg(ctx, frame, inner, &ty, expr.span),
        TypedExprKind::Not(inner) => arith::not(ctx, frame, inner),
        TypedExprKind::Binary { op, lhs, rhs, rhs_hoists, .. } => {
            arith::binary(ctx, frame, *op, lhs, rhs, rhs_hoists, expr.span)
        }
        // Poisoned recovery node; only reachable in programs that failed
        // upstream. `undef` keeps emission total without inventing behavior.
        TypedExprKind::Invalid => undef_of(ctx, &ty),
        // -- v0.0.6 (§9.9a/§9.10) ----------------------------------------
        TypedExprKind::NoneLit => tagged::tagged_none_const(ctx, &ty),
        TypedExprKind::SomeLit(value) => {
            let payload = emit(ctx, frame, value);
            let agg = crate::types::to_llvm(ctx, &ty).into_struct_type().get_undef();
            let with_tag = ctx
                .builder
                .build_insert_value(agg, tagged::i64_const(ctx, 0), 0, "some.tag")
                .expect("insert tag");
            ctx.builder
                .build_insert_value(with_tag, payload, 1, "some.payload")
                .expect("insert payload")
                .into_struct_value()
                .into()
        }
        TypedExprKind::OkLit(value) => {
            let payload = emit(ctx, frame, value);
            let struct_ty = crate::types::to_llvm(ctx, &ty).into_struct_type();
            let agg = struct_ty.get_undef();
            let with_tag = ctx
                .builder
                .build_insert_value(agg, tagged::i64_const(ctx, 0), 0, "ok.tag")
                .expect("insert tag");
            let with_ok = ctx
                .builder
                .build_insert_value(with_tag, payload, 1, "ok.payload")
                .expect("insert ok");
            let err_field = struct_ty
                .get_field_type_at_index(2)
                .expect("err field");
            let zero = tagged::zero_of(ctx, err_field);
            ctx.builder
                .build_insert_value(with_ok, zero, 2, "ok.err.zero")
                .expect("insert err zero")
                .into_struct_value()
                .into()
        }
        TypedExprKind::ErrLit(value) => {
            let payload = emit(ctx, frame, value);
            let struct_ty = crate::types::to_llvm(ctx, &ty).into_struct_type();
            let agg = struct_ty.get_undef();
            let with_tag = ctx
                .builder
                .build_insert_value(agg, tagged::i64_const(ctx, 1), 0, "err.tag")
                .expect("insert tag");
            let ok_field = struct_ty
                .get_field_type_at_index(1)
                .expect("ok field");
            let zero = tagged::zero_of(ctx, ok_field);
            let with_ok_zero = ctx
                .builder
                .build_insert_value(with_tag, zero, 1, "err.ok.zero")
                .expect("insert ok zero");
            ctx.builder
                .build_insert_value(with_ok_zero, payload, 2, "err.payload")
                .expect("insert err")
                .into_struct_value()
                .into()
        }
        // `lhs ?? rhs` — the rhs evaluates ONLY when lhs is inactive (§9.9a
        // laziness). The result flows through an entry slot so both branches
        // join without a phi; ownership follows the active branch (the pass
        // treats the result as borrowed — see the ownership commit).
        TypedExprKind::Coalesce { lhs, rhs } => {
            tagged::lazy_select(ctx, frame, lhs, rhs, &ty)
        }
        TypedExprKind::UnwrapOr { receiver, default } => {
            tagged::lazy_select(ctx, frame, receiver, default, &ty)
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
                .build_int_compare(inkwell::IntPredicate::EQ, tag, tagged::i64_const(ctx, 0), "is.ok")
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
                if tagged::terminated_here(ctx) {
                    break;
                }
            }
            if !tagged::terminated_here(ctx) {
                let tail_v = emit(ctx, frame, tail);
                let _ = ctx.builder.build_store(slot, tail_v);
                for (local, rty) in releases.iter() {
                    crate::emit::ownership::emit_release_slot(ctx, rty, frame.slot(*local));
                }
                let _ = ctx.builder.build_unconditional_branch(join_bb);
            }

            // Always continue emission in the join block: both predecessors
            // end in unconditional branches, so the builder's cursor would
            // otherwise stay parked at the end of a TERMINATED block and any
            // subsequent instructions would land as dead code after a
            // terminator — leaving join itself empty (v0.0.8.1 BUG-4).
            ctx.builder.position_at_end(join_bb);
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
        TypedExprKind::ClosureLit(clo) => closure::emit_closure(ctx, frame, clo, &ty),
        TypedExprKind::Call { func, args } => heap::call(ctx, frame, *func, args),
        TypedExprKind::FieldAccess {
            base,
            struct_id,
            field,
        } => heap::field_read(ctx, frame, base, *struct_id, *field, &ty),
        TypedExprKind::StructLit { struct_id, values } => {
            heap::struct_lit(ctx, frame, *struct_id, values)
        }
        TypedExprKind::StrLit { value } => {
            let base = heap::string_lit(ctx, value);
            // `let w: string @wallclock(d) = "...";` (§5.1.7): the literal is
            // the INNER representation; construction must allocate the
            // unconditionally-heap wallclock HEADER around it — storing the
            // bare string pointer here made every later release type-confuse
            // a KaiString as a KaiWallclock.
            if let KaiType::Temporal {
                inner,
                origin: kai_tast::TemporalOrigin::Wallclock,
                ..
            } = &expr.ty
            {
                crate::emit::wallclock::wallclock_construct(ctx, inner, base)
            } else {
                base
            }
        }
        TypedExprKind::ArrayLit { elements } => {
            let elem = match &expr.ty {
                KaiType::Array(elem) => elem.as_ref().clone(),
                other => unreachable!("array literal typed {other:?}"),
            };
            heap::array_lit(ctx, frame, elements, &elem)
        }
        TypedExprKind::Index { base, index } => {
            heap::index_read(ctx, frame, base, index, &ty, expr.span)
        }
        // Ownership marker from the ownership pass (§9.5): the inner value
        // is borrowed and entering an owning slot. Headers get one refcount
        // op; heap-bearing structs get per-field retains at their source
        // place, then a bitwise copy flows onward.
        TypedExprKind::Retain(inner) => match &expr.ty {
            KaiType::Temporal { inner: t_inner, origin, .. } => match origin {
                // §5.1.7: @wallclock co-ownership rides the HEADER refcount
                // (single rc covering the whole aggregate — the dtor
                // cascades into the inner exactly once at rc==0). Retaining
                // only the inner here would leave two bindings sharing one
                // header with rc still 1 → double header release.
                kai_tast::TemporalOrigin::Wallclock => {
                    let value = emit(ctx, frame, inner);
                    crate::emit::ownership::retain_header(ctx, value);
                    value
                }
                kai_tast::TemporalOrigin::Local => match t_inner.as_ref() {
                    KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                        let value = emit(ctx, frame, inner);
                        let env = if matches!(t_inner.as_ref(), KaiType::Closure { .. }) {
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
                    KaiType::Struct(_) | KaiType::Optional(_) | KaiType::Result { .. } => {
                        let value = emit(ctx, frame, inner);
                        let agg_ty = to_llvm(ctx, &expr.ty);
                        let tmp = crate::emit::alloca_in_entry(
                            ctx,
                            crate::emit::current_function(ctx),
                            agg_ty,
                            "retain.tmp",
                        );
                        let _ = ctx.builder.build_store(tmp, value);
                        if matches!(t_inner.as_ref(), KaiType::Struct(_)) {
                            crate::emit::ownership::retain_struct_copy(ctx, t_inner, tmp);
                        } else {
                            crate::emit::ownership_tagged::retain_tagged_copy(ctx, t_inner, tmp);
                        }
                        ctx.builder
                            .build_load(agg_ty, tmp, "retained.v")
                            .expect("load retained")
                    }
                    other => unreachable!("retain of non-heap temporal-local inner {other:?}"),
                },
            },
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
                    crate::emit::ownership_tagged::retain_tagged_copy(ctx, &expr.ty, tmp);
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
pub(crate) fn undef_of<'ctx>(ctx: &Ctx<'ctx>, ty: &KaiType) -> BasicValueEnum<'ctx> {
    match crate::types::to_llvm(ctx, ty) {
        inkwell::types::BasicTypeEnum::IntType(int_ty) => int_ty.get_undef().into(),
        inkwell::types::BasicTypeEnum::FloatType(float_ty) => float_ty.get_undef().into(),
        _ => unreachable!("scalar types only"),
    }
}

pub(crate) fn int_const<'ctx>(ctx: &Ctx<'ctx>, value: i64, ty: &KaiType) -> IntValue<'ctx> {
    let int_ty = match *ty {
        KaiType::Int64 => ctx.context.i64_type(),
        _ => ctx.context.i32_type(),
    };
    // `true` = signed interpretation of the two's-complement pattern.
    int_ty.const_int(value as u64, true)
}

pub(crate) fn load_local<'ctx>(
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

