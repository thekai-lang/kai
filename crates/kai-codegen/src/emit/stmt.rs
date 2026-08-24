//! Statement emission: returns, bindings (alloca + store), assignment
//! (read-modify-write for compound ops), if/else branching, nested blocks.

use crate::context::Ctx;
use crate::emit::expr;
use crate::frame::Frame;
use crate::types;
use inkwell::basic_block::BasicBlock;
use kai_tast::{TypedAssign, TypedFor, TypedIf, TypedLet, TypedStmt};

pub(crate) fn emit<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, stmt: &TypedStmt) {
    match stmt {
        TypedStmt::Return(value) => ret(ctx, frame, value.as_ref()),
        TypedStmt::Let(binding) => let_stmt(ctx, frame, binding),
        TypedStmt::Assign(assign) => assign_stmt(ctx, frame, assign),
        TypedStmt::If(if_) => if_stmt(ctx, frame, if_),
        TypedStmt::Block(block) => {
            for inner in &block.stmts {
                emit(ctx, frame, inner);
            }
        }
        TypedStmt::Expr(e) => {
            // Value discarded; calls make this meaningful in v0.0.3.
            let _ = expr::emit(ctx, frame, e);
        }
        TypedStmt::For(f) => for_stmt(ctx, frame, f),
        // Ownership marker from the pass: the local's heap content leaves
        // scope here (§9.4). The slot points at storage of `ty`.
        TypedStmt::ReleaseLocal { local, ty } => {
            let slot = frame.slot(*local);
            crate::emit::ownership::emit_release_slot(ctx, ty, slot);
        }
        TypedStmt::ReturnCleanup { value, releases } => {
            // Value first: it may read locals that are about to be
            // released (the §9.5 retain already protected heap content).
            let value = value.as_ref().map(|e| expr::emit(ctx, frame, e));
            for (local, ty) in releases.iter() {
                let slot = frame.slot(*local);
                crate::emit::ownership::emit_release_slot(ctx, ty, slot);
            }
            let _ = ctx
                .builder
                .build_return(value.as_ref().map(|v| v as &dyn inkwell::values::BasicValue<'_>));
        }
    }
}

/// `for name in array { body }`: classic induction over the header's len.
/// The binding slot is written fresh each iteration from the element slot —
/// the loop variable borrows one element at a time (§9.9); no retain yet,
/// that lands with the ownership pass.
fn for_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, f: &TypedFor) {
    let header = match expr::emit(ctx, frame, &f.iterable) {
        inkwell::values::BasicValueEnum::PointerValue(p) => p,
        _ => unreachable!("for iterable is always a header pointer"),
    };

    let elem_kai_ty = match &f.iterable.ty {
        kai_tast::KaiType::Array(elem) => elem.as_ref().clone(),
        other => unreachable!("for iterable typed {other:?}"),
    };
    let elem_llvm = types::to_llvm(ctx, &elem_kai_ty);
    let len = expr::header_len(ctx, header, elem_llvm);

    let function = super::current_function(ctx);
    let binding_slot =
        super::alloca_in_entry(ctx, function, elem_llvm, &f.binding_name);
    frame.bind(f.binding_local, binding_slot);
    let idx_slot = super::alloca_in_entry(ctx, function, ctx.context.i64_type().into(), "for.idx");
    let zero64 = ctx.context.i64_type().const_zero();
    let _ = ctx.builder.build_store(idx_slot, zero64);

    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    let _ = &current;
    let cond_bb = ctx.context.append_basic_block(function, "for.cond");
    let body_bb = ctx.context.append_basic_block(function, "for.body");
    let end_bb = ctx.context.append_basic_block(function, "for.end");
    let _ = ctx.builder.build_unconditional_branch(cond_bb);

    ctx.builder.position_at_end(cond_bb);
    let i = ctx
        .builder
        .build_load(ctx.context.i64_type(), idx_slot, "for.i")
        .expect("idx load")
        .into_int_value();
    let more = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::SLT, i, len, "for.more")
        .expect("icmp");
    let _ = ctx
        .builder
        .build_conditional_branch(more, body_bb, end_bb);

    ctx.builder.position_at_end(body_bb);
    let elems = expr::elems_storage_of(ctx, header, elem_llvm);
    let elem_slot = unsafe {
        ctx.builder
            .build_in_bounds_gep(elem_llvm, elems, &[i], "for.elem.slot")
            .expect("element gep")
    };
    let elem = ctx
        .builder
        .build_load(elem_llvm, elem_slot, "for.elem")
        .expect("element load");
    let _ = ctx.builder.build_store(binding_slot, elem);
    for inner in &f.body.stmts {
        emit(ctx, frame, inner);
    }
    // Back edge only when the body didn't already diverge.
    if ctx
        .builder
        .get_insert_block()
        .and_then(|b| b.get_terminator())
        .is_none()
    {
        let next = ctx
            .builder
            .build_int_add(i, ctx.context.i64_type().const_int(1, false), "for.next")
            .expect("iadd");
        let _ = ctx.builder.build_store(idx_slot, next);
        let _ = ctx.builder.build_unconditional_branch(cond_bb);
    }

    ctx.builder.position_at_end(end_bb);
    // Owned temporary iterables transfer into the loop machinery (§9.9):
    // release the header now that iteration is done. Borrowed iterables
    // stay owned by their source binding — nothing to do.
    if f.iterable_owned {
        crate::emit::ownership::release_header_value(
            ctx,
            inkwell::values::BasicValueEnum::PointerValue(header),
        );
    }
}

fn ret<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, value: Option<&kai_tast::TypedExpr>) {
    match value {
        Some(e) => {
            let value = expr::emit(ctx, frame, e);
            let _ = ctx.builder.build_return(Some(&value));
        }
        None => {
            let _ = ctx.builder.build_return(None);
        }
    }
}

fn let_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, binding: &TypedLet) {
    let value = expr::emit(ctx, frame, &binding.init);
    let function = super::current_function(ctx);

    let slot = super::alloca_in_entry(ctx, function, value.get_type(), &binding.name);
    let _ = ctx.builder.build_store(slot, value);
    frame.bind(binding.local, slot);
}

fn assign_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, assign: &TypedAssign) {
    // Resolve the place. Field hops GEP into inline struct memory; index
    // hops deref the header pointer the current slot holds and GEP into
    // element storage. The index expression re-evaluates at THIS site —
    // it rides in the step (§9.3).
    let mut ptr = frame.slot(assign.root);
    for step in &assign.path {
        ptr = match step {
            kai_tast::TypedPlaceStep::Field(fs) => {
                super::field_gep(ctx, fs.struct_id, ptr, u32::from(fs.field), "place")
            }
            kai_tast::TypedPlaceStep::Index(index) => {
                let elem_ty = types::to_llvm(ctx, &assign.value.ty);
                let header = expr::header_of_value(
                    ctx.builder
                        .build_load(
                            ctx.context.ptr_type(Default::default()),
                            ptr,
                            "arr.hdr",
                        )
                        .expect("array value load"),
                );
                let idx64 = expr::widen_index(ctx, expr::emit(ctx, frame, index).into_int_value());
                expr::elem_slot(
                    ctx,
                    frame,
                    index.span,
                    header,
                    elem_ty,
                    idx64,
                    "place.elem",
                )
            }
        };
    }

    // Prepare the replacement FIRST — the RHS may alias the destination
    // (`arr[0] = arr[0]`), so nothing at the destination may be released
    // before the new value fully exists (§9.4 ordering).
    let value = expr::emit(ctx, frame, &assign.value);

    match assign.op {
        Some(op) => {
            // Compound ops exist only on numeric slots in v0.0.5; no
            // ownership event, straight read-modify-write.
            let pointee = types::to_llvm(ctx, &assign.value.ty);
            let old = ctx
                .builder
                .build_load(pointee, ptr, "old")
                .expect("load for compound assign");
            let combined = expr::apply_binary(ctx, frame, op, old, value, &assign.value.ty, assign.span);
            let _ = ctx.builder.build_store(ptr, combined);
        }
        None => {
            if assign.release_old {
                crate::emit::ownership::emit_release_slot(ctx, &assign.value.ty, ptr);
            }
            let _ = ctx.builder.build_store(ptr, value);
        }
    }
}

fn if_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, if_: &TypedIf) {
    let cond = expr::emit(ctx, frame, &if_.cond).into_int_value();

    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    let function = current.get_parent().expect("function");

    let then_bb = ctx.context.append_basic_block(function, "if.then");
    let else_bb = if_
        .else_block
        .as_ref()
        .map(|_| ctx.context.append_basic_block(function, "if.else"));
    let merge_bb = ctx.context.append_basic_block(function, "if.end");

    let _ = ctx
        .builder
        .build_conditional_branch(cond, then_bb, else_bb.unwrap_or(merge_bb));

    ctx.builder.position_at_end(then_bb);
    for inner in &if_.then_block.stmts {
        emit(ctx, frame, inner);
    }
    branch_to(ctx, merge_bb);

    if let (Some(block), Some(bb)) = (if_.else_block.as_ref(), else_bb) {
        ctx.builder.position_at_end(bb);
        for inner in &block.stmts {
            emit(ctx, frame, inner);
        }
        branch_to(ctx, merge_bb);
    }

    ctx.builder.position_at_end(merge_bb);
}

/// Branches to `target` unless the block already ended (e.g. an arm whose
/// every path returned). LLVM would otherwise append a second terminator,
/// which fails module verification.
fn branch_to<'ctx>(ctx: &Ctx<'ctx>, target: BasicBlock<'ctx>) {
    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    if current.get_terminator().is_some() {
        return;
    }
    let _ = ctx.builder.build_unconditional_branch(target);
}

/// Emits a return for any block left unterminated by control flow (e.g. an
/// `if/else` where both arms returned, followed by unreachable statements).
pub(crate) fn fallback_return<'ctx>(ctx: &Ctx<'ctx>, ret: &kai_tast::KaiType) {
    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    if current.get_terminator().is_some() {
        return;
    }
    match types::zero_of(ctx, ret) {
        Some(zero) => {
            let _ = ctx.builder.build_return(Some(&zero));
        }
        None => {
            let _ = ctx.builder.build_return(None);
        }
    }
}
