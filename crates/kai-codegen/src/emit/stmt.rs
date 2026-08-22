//! Statement emission: returns, bindings (alloca + store), assignment
//! (read-modify-write for compound ops), if/else branching, nested blocks.

use crate::context::Ctx;
use crate::emit::expr;
use crate::frame::Frame;
use crate::types;
use inkwell::basic_block::BasicBlock;
use kai_tast::{TypedAssign, TypedIf, TypedLet, TypedStmt};

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
    }
}

fn ret<'ctx>(ctx: &Ctx<'ctx>, frame: &Frame<'ctx>, value: Option<&kai_tast::TypedExpr>) {
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
    let slot = ctx
        .builder
        .build_alloca(value.get_type(), &binding.name)
        .expect("alloca for local");
    let _ = ctx.builder.build_store(slot, value);
    frame.bind(binding.local, slot);
}

fn assign_stmt<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, assign: &TypedAssign) {
    // Strict same-type rules guarantee value.ty == the target's type, so it
    // doubles as the operand type for compound read-modify-write.
    let value = expr::emit(ctx, frame, &assign.value);

    let value = match assign.op {
        Some(op) => {
            let slot = frame.slot(assign.local);
            let pointee = types::to_llvm(ctx, assign.value.ty);
            let old = ctx
                .builder
                .build_load(pointee, slot, "old")
                .expect("load for compound assign");
            expr::apply_binary(ctx, op, old, value, assign.value.ty)
        }
        None => value,
    };

    let slot = frame.slot(assign.local);
    let _ = ctx.builder.build_store(slot, value);
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
pub(crate) fn fallback_return<'ctx>(ctx: &Ctx<'ctx>, ret: kai_tast::KaiType) {
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
