#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Arithmetic emission helpers.
use crate::context::Ctx;
use crate::frame::Frame;
use inkwell::basic_block::BasicBlock;
use inkwell::values::{BasicValueEnum, FloatValue, IntValue, ValueKind};
use kai_tast::{BinaryOp, KaiType, TypedExpr};

use super::{call_value, emit, int_const};

pub(crate) fn neg<'ctx>(
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

pub(crate) fn not<'ctx>(ctx: &Ctx<'ctx>, frame: &mut Frame<'ctx>, operand: &TypedExpr) -> BasicValueEnum<'ctx> {
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

pub(crate) fn overflow_intrinsic<'ctx>(
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

pub(crate) fn checked_arith<'ctx>(
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

pub(crate) fn div_guard<'ctx>(
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

pub(crate) fn int_arith<'ctx>(
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

pub(crate) fn float_arith<'ctx>(
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

pub(crate) fn binary<'ctx>(
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

pub(crate) fn short_circuit<'ctx>(
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

