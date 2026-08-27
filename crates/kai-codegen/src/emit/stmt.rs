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
        TypedStmt::Require(e) => {
            // v0.0.8 (§5.2.1 + §10.3): evaluate exactly once, synchronously;
            // on violation record to the pre-ledger sink FIRST (§10.3
            // sequencing), then panic with the raw source-text condition.
            // String-API builds skip only the record — the panic itself is
            // unconditional behavior, not part of the documented no-op.
            let cond_value = expr::emit(ctx, frame, e);
            let cond = cond_value.into_int_value();
            let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
            let function = current.get_parent().expect("function");
            let ok_bb = ctx.context.append_basic_block(function, "require.ok");
            let viol_bb = ctx.context.append_basic_block(function, "require.viol");

            let _ = ctx
                .builder
                .build_conditional_branch(cond, ok_bb, viol_bb)
                .expect("require branch");

            ctx.builder.position_at_end(viol_bb);
            if let Some(record) = signal_record(ctx, frame, e.span, "debt.log") {
                let args = [
                    record.sink.into(),
                    record.location.into(),
                    record.condition.into(),
                ];
                ctx.builder
                    .build_call(
                        crate::runtime::observe::debt_record_fn(ctx),
                        &args,
                        "debt.rec",
                    )
                    .expect("kai_debt_record call");
            }
            let cond_text = condition_text(ctx, &frame.module, e.span);
            let msg = format!("requirement violated: {cond_text}");
            let msg_global = ctx
                .builder
                .build_global_string_ptr(&msg, "kai.require.msg")
                .expect("require message global");
            let file = crate::emit::panic::file_global_for(ctx, &frame.module);
            let (line, col) = ctx
                .sources
                .get(&frame.module)
                .map_or((0, 0), |src| src.line_col(e.span.start));
            let i64_ty = ctx.context.i64_type();
            // §5.3 unwind before the terminal §10.1 require-panic: roll back
            // the current reversible activation's ledger (restore Places +
            // release displaced values). Only inside `reversible` functions.
            crate::emit::reversible::unwind_if_active(ctx);
            let panic_args = [
                msg_global.as_pointer_value().into(),
                i64_ty.const_int(msg.len() as u64, false).into(),
                file.into(),
                i64_ty.const_int(line as u64, false).into(),
                i64_ty.const_int(col as u64, false).into(),
            ];
            ctx.builder
                .build_call(crate::runtime::panic_fn(ctx), &panic_args, "require.panic")
                .expect("kai_panic call");
            ctx.builder
                .build_unreachable()
                .expect("panic never returns");

            ctx.builder.position_at_end(ok_bb);
        }
        TypedStmt::Observe(e) => {
            // v0.0.8 (§5.2.2): Signal telemetry — evaluate exactly once,
            // record {timestamp, location, condition, outcome}, never fatal.
            // Root-less string-API builds skip recording entirely (documented
            // no-op, v0.21); evaluation itself is unchanged.
            let outcome_bool = expr::emit(ctx, frame, e);
            let outcome = ctx.builder.build_int_cast(
                outcome_bool.into_int_value(),
                ctx.context.i32_type(),
                "observe.outcome.i32",
            ).expect("outcome widen to i32");
            if let Some(record) = signal_record(ctx, frame, e.span, "observe.log") {
                let args = [
                    record.sink.into(),
                    record.location.into(),
                    record.condition.into(),
                    outcome.into(),
                ];
                ctx.builder
                    .build_call(
                        crate::runtime::observe::observe_record_fn(ctx),
                        &args,
                        "observe.rec",
                    )
                    .expect("kai_observe_record call");
            }
        }
        TypedStmt::Expr(e) => {
            // Value discarded; calls make this meaningful in v0.0.3.
            let _ = expr::emit(ctx, frame, e);
        }
        TypedStmt::For(f) => for_stmt(ctx, frame, f),
        TypedStmt::While(w) => while_stmt(ctx, frame, w),
        // §5.3.1 ledger push: resolves the Place (same root/path as the
        // following Assign), loads the OLD value, retains it if heap-bearing
        // (snapshot owns the claim), then appends to the current activation's
        // ledger (§5.3.5). E2 snapshot emission.
        TypedStmt::ReversiblePush(push) => crate::emit::reversible::emit_push(ctx, frame, push),
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
            crate::emit::reversible::commit_if_reversible(ctx, frame);
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
    crate::emit::reversible::commit_if_reversible(ctx, frame);
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
    let ptr = resolve_place(ctx, frame, assign.root, &assign.path, &assign.value.ty);

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

/// Resolves a Place (`root` + `path`) to the concrete storage slot a mutation
/// reads/writes. Used identically by `Assign` and the reversible `LedgerPush`
/// that precedes it, so both snapshot and store hit the SAME element — index
/// expressions (pure in Kai) re-evaluate to the same slot with no intervening
/// store (§5.3 audit: the ledger stores this resolved slot pointer, so unwind
/// never re-evaluates the index).
pub(crate) fn resolve_place<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &mut Frame<'ctx>,
    root: kai_tast::LocalId,
    path: &[kai_tast::TypedPlaceStep],
    value_ty: &kai_tast::KaiType,
) -> inkwell::values::PointerValue<'ctx> {
    let mut ptr = frame.slot(root);
    for step in path {
        ptr = match step {
            kai_tast::TypedPlaceStep::Field(fs) => {
                super::field_gep(ctx, fs.struct_id, ptr, u32::from(fs.field), "place")
            }
            kai_tast::TypedPlaceStep::Index(index) => {
                let elem_ty = types::to_llvm(ctx, value_ty);
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
    ptr
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
pub(crate) fn fallback_return<'ctx>(
    ctx: &Ctx<'ctx>,
    ret: &kai_tast::KaiType,
    frame: &Frame<'ctx>,
) {
    let current: BasicBlock = ctx.builder.get_insert_block().expect("insert position");
    if current.get_terminator().is_some() {
        return;
    }
    crate::emit::reversible::commit_if_reversible(ctx, frame);
    match types::zero_of(ctx, ret) {
        Some(zero) => {
            let _ = ctx.builder.build_return(Some(&zero));
        }
        None => {
            let _ = ctx.builder.build_return(None);
        }
    }
}


// -- v0.0.8 §5.2 signal recording helpers --------------------------------------

/// One baked signal call's arguments: the sink path (project-root-relative
/// `.kai/*.log`), the `file:line:col` location, and the raw source-text
/// condition (v0.22). `None` = root-less string API — documented recording
/// no-op (§5.2.2/v0.21).
struct SignalRecord<'ctx> {
    #[allow(dead_code)]
    sink: inkwell::values::PointerValue<'ctx>,
    location: inkwell::values::PointerValue<'ctx>,
    condition: inkwell::values::PointerValue<'ctx>,
}

/// Bakes the three c-string globals for a require/observe record, or returns
/// `None` when compiling via the root-less string API. `sink_file` is
/// `"observe.log"` or `"debt.log"`, resolved under `<root>/.kai/`.
fn signal_record<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &crate::frame::Frame<'ctx>,
    span: kai_diagnostics::Span,
    sink_file: &str,
) -> Option<SignalRecord<'ctx>> {
    let root = ctx.sink_root.as_ref()?;
    let info = ctx.sources.get(&frame.module)?;
    let location = info.location(span);
    let condition = info.slice(span);

    let sink_path = root.join(".kai").join(sink_file);
    let sink_str = sink_path.to_string_lossy().into_owned();

    Some(SignalRecord {
        sink: bake_cstr(ctx, &sink_str, "kai.sink.path"),
        location: bake_cstr(ctx, &location, "kai.sig.loc"),
        condition: bake_cstr(ctx, &condition, "kai.sig.cond"),
    })
}

/// Private unnamed constant byte-string with NUL terminator, as an i8*.
fn bake_cstr<'ctx>(
    ctx: &Ctx<'ctx>,
    text: &str,
    name: &str,
) -> inkwell::values::PointerValue<'ctx> {
    let global = ctx
        .builder
        .build_global_string_ptr(text, name)
        .expect("string global");
    global.as_pointer_value()
}


/// §5.2/v0.22: condition text is the raw source-text span — verbatim slice,
/// never an AST pretty-print. Falls back to `<unknown>` when no source is
/// attached (tests that pass empty source lists).
fn condition_text<'ctx>(ctx: &Ctx<'ctx>, module_key: &str, span: kai_diagnostics::Span) -> String {
    ctx.sources
        .get(module_key)
        .map_or_else(|| "<unknown>".to_string(), |src| src.slice(span))
}

/// `while cond { body }` (v0.0.8.1): classic cond/body/end blocks.
/// `cond_prelude` (hidden temporaries) is emitted at the TOP of cond so
/// the condition evaluates FRESH every iteration; `cond_releases` ride
/// BOTH the back-edge and loop exit — one release per evaluation, never
/// zero (§5.2-style dual-point care applied to loop conditions).
fn while_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, w: &kai_tast::TypedWhile) {
    let function = super::current_function(ctx);
    let cond_bb = ctx.context.append_basic_block(function, "while.cond");
    let body_bb = ctx.context.append_basic_block(function, "while.body");
    let end_bb = ctx.context.append_basic_block(function, "while.end");

    let _ = ctx.builder.build_unconditional_branch(cond_bb);
    ctx.builder.position_at_end(cond_bb);
    for inner in &w.cond_prelude {
        emit(ctx, frame, inner);
    }
    let cond = expr::emit(ctx, frame, &w.cond).into_int_value();
    let _ = ctx.builder.build_conditional_branch(cond, body_bb, end_bb);

    ctx.builder.position_at_end(body_bb);
    for inner in &w.body.stmts {
        emit(ctx, frame, inner);
    }
    // Back-edge: release this iteration's condition temporaries first.
    for (local, ty) in w.cond_releases.iter() {
        crate::emit::ownership::emit_release_slot(ctx, ty, frame.slot(*local));
    }
    branch_to(ctx, cond_bb);

    ctx.builder.position_at_end(end_bb);
    // Loop exit: final evaluation's temporaries released here.
    for (local, ty) in w.cond_releases.iter() {
        crate::emit::ownership::emit_release_slot(ctx, ty, frame.slot(*local));
    }
}
