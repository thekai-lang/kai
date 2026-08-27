//! Compensate expression emission.
//! Generates the environment and the runtime thunk for `compensate` blocks.

use crate::context::Ctx;
use crate::frame::Frame;
use crate::types::to_llvm;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum};
use kai_tast::{KaiType, TypedCapture, TypedStmt};
use crate::emit::stmt;

pub(crate) fn emit_compensate<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    base: &kai_tast::TypedExpr,
    stmts: &[TypedStmt],
    captures: &[TypedCapture],
    ty: &KaiType,
) -> BasicValueEnum<'ctx> {
    let seq = ctx.closure_seq.get();
    ctx.closure_seq.set(seq + 1);

    // 1. P2.2: Environment Lowering
    let mut field_tys: Vec<BasicTypeEnum<'ctx>> = captures
        .iter()
        .map(|c| to_llvm(ctx, &c.ty))
        .collect();
    
    let caps_ty = {
        let name = format!("KaiCompensateEnv.{seq}");
        if let Some(existing) = ctx.module.get_struct_type(&name) {
            existing
        } else {
            let t = ctx.context.opaque_struct_type(&name);
            t.set_body(&field_tys, false);
            t
        }
    };
    let caps_size = caps_ty.size_of().expect("sized captures");

    // Allocate environment temporarily on the stack (it gets copied by value in push_compensate)
    let env_ptr = crate::emit::alloca_in_entry(ctx, crate::emit::current_function(ctx), caps_ty.into(), "comp.env");
    
    // Populate the environment and perform Retain (P2.3)
    let mut any_heap = false;
    for (i, cap) in captures.iter().enumerate() {
        if crate::emit::ownership::heap_bearing(ctx, &cap.ty) {
            any_heap = true;
        }
        let src = frame.slot(cap.local);
        let dst = ctx.builder.build_struct_gep(caps_ty, env_ptr, i as u32, "cap.dst").expect("gep cap");

        match &cap.ty {
            KaiType::String | KaiType::Array(_) | KaiType::Closure { .. } => {
                let v = ctx.builder.build_load(to_llvm(ctx, &cap.ty), src, "cap.v").expect("load capture");
                crate::emit::ownership::retain_header(ctx, v);
                let _ = ctx.builder.build_store(dst, v);
            }
            k if matches!(k, KaiType::Struct(_) | KaiType::Optional(_) | KaiType::Result { .. })
                && crate::emit::ownership::heap_bearing(ctx, k) =>
            {
                let copy = if matches!(k, KaiType::Struct(_)) {
                    crate::emit::ownership::retain_struct_copy(ctx, k, src)
                } else {
                    crate::emit::ownership_tagged::retain_tagged_copy(ctx, k, src)
                };
                let _ = ctx.builder.build_store(dst, copy);
            }
            _ => {
                let v = ctx.builder.build_load(to_llvm(ctx, &cap.ty), src, "cap.v").expect("load capture");
                let _ = ctx.builder.build_store(dst, v);
            }
        }
    }

    // 2. Build the Release Dtor (P2.3)
    let dtor_val = if any_heap {
        let dname = format!("kai.dtor.comp.{seq}");
        let ptr = ctx.context.ptr_type(Default::default());
        let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
        let dtor = ctx.module.add_function(&dname, llvm, None);
        let saved_block = ctx.builder.get_insert_block();
        let entry = ctx.context.append_basic_block(dtor, "entry");
        ctx.builder.position_at_end(entry);
        
        let env_arg = dtor.get_nth_param(0).expect("env").into_pointer_value();
        
        for (i, cap) in captures.iter().enumerate() {
            if crate::emit::ownership::heap_bearing(ctx, &cap.ty) {
                let field_ptr = ctx.builder.build_struct_gep(caps_ty, env_arg, i as u32, "dtor.p").expect("dtor gep");
                crate::emit::ownership::emit_release_slot(ctx, &cap.ty, field_ptr);
            }
        }
        let _ = ctx.builder.build_return(None);
        ctx.builder.position_at_end(saved_block.expect("saved block"));
        dtor.as_global_value().as_pointer_value()
    } else {
        ctx.context.ptr_type(Default::default()).const_null()
    };

    // 3. Build the Thunk (P2.4)
    let tname = format!("kai.thunk.comp.{seq}");
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into()], false);
    let thunk = ctx.module.add_function(&tname, llvm, None);
    let saved_block = ctx.builder.get_insert_block();
    let entry = ctx.context.append_basic_block(thunk, "entry");
    ctx.builder.position_at_end(entry);
    
    let env_arg = thunk.get_nth_param(0).expect("env").into_pointer_value();
    
    // Create a new frame for the thunk
    let mut thunk_frame = Frame::new(frame.module.clone());
    // Map captured variables to the fields inside `env_arg`
    for (i, cap) in captures.iter().enumerate() {
        let field_ptr = ctx.builder.build_struct_gep(caps_ty, env_arg, i as u32, "thunk.cap.p").expect("thunk gep");
        thunk_frame.bind(cap.local, field_ptr);
    }
    
    // Emit the statements inside the thunk
    for s in stmts {
        stmt::emit(ctx, &mut thunk_frame, s);
    }
    let _ = ctx.builder.build_return(None);
    ctx.builder.position_at_end(saved_block.expect("saved block"));
    let thunk_val = thunk.as_global_value().as_pointer_value();

    // 4. Register to the Reversible Runtime (P2.5)
    let args = [
        env_ptr.into(),
        caps_size.into(),
        thunk_val.into(),
        dtor_val.into(),
    ];
    ctx.builder.build_call(crate::runtime::reversible::reversible_push_compensate_fn(ctx), &args, "rev.push.comp").expect("kai_reversible_push_compensate");

    // Finally, evaluate and return the base call expression
    super::emit(ctx, frame, base)
}
