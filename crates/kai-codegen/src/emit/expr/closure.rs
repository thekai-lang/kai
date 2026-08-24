#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Closure emission — environment heap, captures, body function.
use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, ValueKind};
use kai_tast::{KaiType, TypedClosure};

use super::i64_const;

pub(crate) fn emit_closure<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, clo: &TypedClosure, ty: &KaiType) -> BasicValueEnum<'ctx> {
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
