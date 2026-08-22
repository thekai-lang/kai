//! Expression emission. Arithmetic/comparison dispatch on the static Kai
//! type; `&&`/`||` lower to short-circuit control flow with phi merges.

use crate::context::Ctx;
use crate::frame::Frame;
use inkwell::basic_block::BasicBlock;
use inkwell::values::{BasicValueEnum, FloatValue, IntValue};
use kai_tast::{BinaryOp, KaiType, TypedExpr, TypedExprKind};

pub(crate) fn emit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &Frame<'ctx>,
    expr: &TypedExpr,
) -> BasicValueEnum<'ctx> {
    match &expr.kind {
        TypedExprKind::IntLit(value) => int_const(ctx, *value, expr.ty).into(),
        TypedExprKind::FloatLit(value) => ctx.context.f64_type().const_float(*value).into(),
        TypedExprKind::BoolLit(value) => ctx
            .context
            .bool_type()
            .const_int(*value as u64, false)
            .into(),
        TypedExprKind::LocalRef(local) => load_local(ctx, frame, *local, expr.ty),
        TypedExprKind::Neg(inner) => neg(ctx, frame, inner, expr.ty),
        TypedExprKind::Not(inner) => not(ctx, frame, inner),
        TypedExprKind::Binary { op, lhs, rhs } => binary(ctx, frame, *op, lhs, rhs),
    }
}

fn int_const<'ctx>(ctx: &Ctx<'ctx>, value: i64, ty: KaiType) -> IntValue<'ctx> {
    let int_ty = match ty {
        KaiType::Int64 => ctx.context.i64_type(),
        _ => ctx.context.i32_type(),
    };
    // `true` = signed interpretation of the two's-complement pattern.
    int_ty.const_int(value as u64, true)
}

fn load_local<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &Frame<'ctx>,
    local: kai_tast::LocalId,
    ty: KaiType,
) -> BasicValueEnum<'ctx> {
    let slot = frame.slot(local);
    let pointee = crate::types::to_llvm(ctx, ty);
    ctx.builder
        .build_load(pointee, slot, "tmp")
        .expect("load from alloca")
}

fn neg<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &Frame<'ctx>,
    operand: &TypedExpr,
    result_ty: KaiType,
) -> BasicValueEnum<'ctx> {
    let value = emit(ctx, frame, operand);
    if result_ty == KaiType::Float64 {
        let as_float = value.into_float_value();
        ctx.builder
            .build_float_neg(as_float, "neg")
            .expect("fneg")
            .into()
    } else {
        let as_int = value.into_int_value();
        let zero = int_const(ctx, 0, result_ty);
        ctx.builder
            .build_int_sub(zero, as_int, "neg")
            .expect("sub for negation")
            .into()
    }
}

fn not<'ctx>(ctx: &Ctx<'ctx>, frame: &Frame<'ctx>, operand: &TypedExpr) -> BasicValueEnum<'ctx> {
    let value = emit(ctx, frame, operand).into_int_value();
    let truth = ctx.context.bool_type().const_int(1, false);
    ctx.builder
        .build_xor(value, truth, "not")
        .expect("xor for !")
        .into()
}
/// Re-usable arithmetic/comparison core on scalars; also used by compound
/// assignment. Dispatches on the static operand type.
pub(crate) fn apply_binary<'ctx>(
    ctx: &Ctx<'ctx>,
    op: BinaryOp,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    operand_ty: KaiType,
) -> BasicValueEnum<'ctx> {
    match operand_ty {
        KaiType::Float64 => float_arith(ctx, op, lhs.into_float_value(), rhs.into_float_value()),
        _ => int_arith(ctx, op, lhs.into_int_value(), rhs.into_int_value()).into(),
    }
}

fn int_arith<'ctx>(
    ctx: &Ctx<'ctx>,
    op: BinaryOp,
    lhs: IntValue<'ctx>,
    rhs: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let b = &ctx.builder;
    match op {
        BinaryOp::Add => b.build_int_add(lhs, rhs, "add").expect("iadd"),
        BinaryOp::Sub => b.build_int_sub(lhs, rhs, "sub").expect("isub"),
        BinaryOp::Mul => b.build_int_mul(lhs, rhs, "mul").expect("imul"),
        BinaryOp::Div => b.build_int_signed_div(lhs, rhs, "div").expect("sdiv"),
        BinaryOp::Mod => b.build_int_signed_rem(lhs, rhs, "rem").expect("srem"),
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
    frame: &Frame<'ctx>,
    op: BinaryOp,
    lhs: &TypedExpr,
    rhs: &TypedExpr,
) -> BasicValueEnum<'ctx> {
    match op {
        // `a && b`: evaluate b only when a is true.
        // `a || b`: evaluate b only when a is false.
        BinaryOp::And => short_circuit(ctx, frame, lhs, rhs, false),
        BinaryOp::Or => short_circuit(ctx, frame, lhs, rhs, true),
        _ => {
            let l = emit(ctx, frame, lhs);
            let r = emit(ctx, frame, rhs);
            apply_binary(ctx, op, l, r, lhs.ty)
        }
    }
}

/// Lowers `lhs OP rhs` for OP in {&&, ||} using explicit branches plus a phi;
/// the rhs side effect (e.g. an assignment-heavy expression later) must not
/// run when short-circuited.
fn short_circuit<'ctx>(
    ctx: &Ctx<'ctx>,
    frame: &Frame<'ctx>,
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
