//! Expression typing. Rules (§9.2): strict same-type arithmetic, `%` ints
//! only, comparisons yield `bool`, `&&`/`||`/`!` are `bool`-only. Integer
//! literals default to `int32` and widen to `int64` only when context demands
//! it (annotation, return type, or the other operand's concrete type).

use crate::checker::Checker;
use crate::error;
use kai_ast::{
    BinaryExpr, BinaryOp as AstBinaryOp, CallExpr, Expr, ExprKind, FieldAccessExpr, Ident,
    StructLitExpr, UnaryOp,
};
use kai_diagnostics::Span;
use kai_tast::{BinaryOp, FunctionId, KaiType, StructId, TypedExpr, TypedExprKind};

/// Lower an AST expression to TAST. `expected` is a width hint for integer
/// literals; it never enables implicit conversions.
pub(crate) fn lower(checker: &mut Checker, expr: &Expr, expected: Option<KaiType>) -> TypedExpr {
    match &expr.kind {
        ExprKind::IntLit(lit) => {
            let span = lit.span;
            int_lit(checker, lit.value, expected, span)
        }
        ExprKind::FloatLit(lit) => {
            TypedExpr::new(TypedExprKind::FloatLit(lit.value), KaiType::Float64)
        }
        ExprKind::BoolLit { value, .. } => {
            TypedExpr::new(TypedExprKind::BoolLit(*value), KaiType::Bool)
        }
        ExprKind::Ident(ident) => ident_ref(checker, ident),
        ExprKind::Unary(unary) => unary_expr(checker, unary.op, &unary.operand),
        ExprKind::Binary(binary) => binary_expr(checker, binary, expected),
        ExprKind::Call(call) => call_expr(checker, call, expr.span),
        ExprKind::FieldAccess(access) => field_access(checker, access),
        ExprKind::StructLit(lit) => struct_lit(checker, lit),
        // Poisoned parser-recovery node. The program already failed upstream;
        // this defensive diagnostic keeps the phase contract explicit.
        ExprKind::Invalid => {
            let span = expr.span;
            checker.error(error::invalid_expression(span));
            TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32)
        }
    }
}

fn int_lit(checker: &mut Checker, value: u64, expected: Option<KaiType>, span: Span) -> TypedExpr {
    let ty = if expected == Some(KaiType::Int64) {
        KaiType::Int64
    } else {
        KaiType::Int32
    };
    let max_inclusive: u64 = match ty {
        KaiType::Int32 => i32::MAX as u64,
        _ => i64::MAX as u64,
    };
    if value > max_inclusive {
        checker.error(error::literal_out_of_range(max_inclusive, ty, span));
    }
    TypedExpr::int_lit(value as i64, ty)
}

fn ident_ref(checker: &mut Checker, ident: &Ident) -> TypedExpr {
    match checker.locals.lookup(&ident.name) {
        Some(info) => TypedExpr::new(TypedExprKind::LocalRef(info.id), info.ty),
        None => {
            let span = ident.span;
            let name = ident.name.clone();
            checker.error(error::undeclared_variable(&name, span));
            // Placeholder keeps compilation going; program is discarded.
            TypedExpr::new(TypedExprKind::IntLit(0), KaiType::Int32)
        }
    }
}

fn unary_expr(checker: &mut Checker, op: UnaryOp, operand: &Expr) -> TypedExpr {
    match op {
        UnaryOp::Neg => {
            // Bare literals fold straight into a negative constant (see
            // `neg_operand`); wrapping them in Neg again would negate twice.
            if matches!(operand.kind, ExprKind::IntLit(_)) {
                return neg_operand(checker, operand);
            }
            let inner = lower(checker, operand, None);
            match inner.ty {
                ty if ty.is_numeric() => TypedExpr::new(TypedExprKind::Neg(Box::new(inner)), ty),
                other => {
                    let span = operand.span;
                    checker.error(error::operand_type_mismatch("-", other, span));
                    TypedExpr::new(TypedExprKind::IntLit(0), KaiType::Int32)
                }
            }
        }
        UnaryOp::Not => {
            let inner = lower(checker, operand, None);
            match inner.ty {
                KaiType::Bool => TypedExpr::new(TypedExprKind::Not(Box::new(inner)), KaiType::Bool),
                other => {
                    let span = operand.span;
                    checker.error(error::operand_type_mismatch("!", other, span));
                    TypedExpr::new(TypedExprKind::BoolLit(false), KaiType::Bool)
                }
            }
        }
    }
}

/// Under `-`, a bare literal may be one past the positive max (`-2147483648`),
/// so its magnitude is checked against max+1 and the result is folded into a
/// negative literal — codegen then never sees an unrepresentable positive
/// constant sitting under a Neg node.
fn neg_operand(checker: &mut Checker, operand: &Expr) -> TypedExpr {
    if let ExprKind::IntLit(lit) = &operand.kind {
        let magnitude = lit.value;
        let (ty, max_magnitude): (KaiType, u64) = if magnitude <= i32::MAX as u64 + 1 {
            (KaiType::Int32, i32::MAX as u64 + 1)
        } else {
            (KaiType::Int64, i64::MAX as u64 + 1)
        };
        if magnitude > max_magnitude {
            let span = lit.span;
            checker.error(error::literal_out_of_range(i64::MAX as u64, ty, span));
        }
        let negated = -(magnitude as i128);
        return TypedExpr::new(TypedExprKind::IntLit(negated as i64), ty);
    }
    lower(checker, operand, None)
}

fn ast_to_tast_op(op: AstBinaryOp) -> BinaryOp {
    match op {
        AstBinaryOp::Add => BinaryOp::Add,
        AstBinaryOp::Sub => BinaryOp::Sub,
        AstBinaryOp::Mul => BinaryOp::Mul,
        AstBinaryOp::Div => BinaryOp::Div,
        AstBinaryOp::Mod => BinaryOp::Mod,
        AstBinaryOp::Lt => BinaryOp::Lt,
        AstBinaryOp::Gt => BinaryOp::Gt,
        AstBinaryOp::Le => BinaryOp::Le,
        AstBinaryOp::Ge => BinaryOp::Ge,
        AstBinaryOp::Eq => BinaryOp::Eq,
        AstBinaryOp::Ne => BinaryOp::Ne,
        AstBinaryOp::And => BinaryOp::And,
        AstBinaryOp::Or => BinaryOp::Or,
    }
}

fn binary_expr(checker: &mut Checker, binary: &BinaryExpr, expected: Option<KaiType>) -> TypedExpr {
    let op = ast_to_tast_op(binary.op);
    let span = binary.rhs.span;

    // Width hints flow into bare literals on either side: the outer context
    // types the left operand, the left operand's resolved type types the right.
    let lhs_hint = match &binary.lhs.kind {
        ExprKind::IntLit(_) => expected.filter(|ty| ty.is_integer()),
        _ => None,
    };
    let lhs = lower(checker, &binary.lhs, lhs_hint);

    let rhs_hint = match &binary.rhs.kind {
        ExprKind::IntLit(_) => Some(lhs.ty).filter(|ty| ty.is_integer()),
        _ => None,
    };
    let rhs = lower(checker, &binary.rhs, rhs_hint);

    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
            arithmetic(checker, op, lhs, rhs, span)
        }
        BinaryOp::Mod => {
            if lhs.ty != rhs.ty || !lhs.ty.is_integer() {
                let s = span;
                checker.error(error::mod_requires_integers(s));
            }
            let result_ty = lhs_placeholder_ty(lhs.ty);
            TypedExpr::new(
                TypedExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                result_ty,
            )
        }
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            comparison(checker, op, lhs, rhs, span)
        }
        BinaryOp::Eq | BinaryOp::Ne => equality(checker, op, lhs, rhs, span),
        BinaryOp::And | BinaryOp::Or => logical(checker, op, lhs, rhs, span),
    }
}

fn arithmetic(
    checker: &mut Checker,
    op: BinaryOp,
    lhs: TypedExpr,
    rhs: TypedExpr,
    span: Span,
) -> TypedExpr {
    let ty = lhs.ty;
    if !ty.is_numeric() || ty != rhs.ty {
        let name = op.describe();
        checker.error(if ty.is_numeric() && rhs.ty.is_numeric() {
            error::binary_type_mismatch(name, ty, rhs.ty, span)
        } else {
            let bad = if ty.is_numeric() { rhs.ty } else { ty };
            error::operand_type_mismatch(name, bad, span)
        });
        return TypedExpr::new(TypedExprKind::IntLit(0), KaiType::Int32);
    }
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty,
    )
}

fn comparison(
    checker: &mut Checker,
    op: BinaryOp,
    lhs: TypedExpr,
    rhs: TypedExpr,
    span: Span,
) -> TypedExpr {
    if !lhs.ty.is_numeric() || lhs.ty != rhs.ty {
        let name = op.describe();
        checker.error(error::binary_type_mismatch(name, lhs.ty, rhs.ty, span));
    }
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        KaiType::Bool,
    )
}

fn equality(
    checker: &mut Checker,
    op: BinaryOp,
    lhs: TypedExpr,
    rhs: TypedExpr,
    span: Span,
) -> TypedExpr {
    if lhs.ty != rhs.ty {
        let name = op.describe();
        checker.error(error::binary_type_mismatch(name, lhs.ty, rhs.ty, span));
    }
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        KaiType::Bool,
    )
}

fn logical(
    checker: &mut Checker,
    op: BinaryOp,
    lhs: TypedExpr,
    rhs: TypedExpr,
    span: Span,
) -> TypedExpr {
    if lhs.ty != KaiType::Bool || rhs.ty != KaiType::Bool {
        let name = op.describe();
        let bad = if lhs.ty == KaiType::Bool {
            rhs.ty
        } else {
            lhs.ty
        };
        checker.error(error::operand_type_mismatch(name, bad, span));
    }
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        KaiType::Bool,
    )
}

/// Result type after a failed `%`: prefer the lhs type when it was an integer.
fn lhs_placeholder_ty(lhs: KaiType) -> KaiType {
    if lhs.is_integer() {
        lhs
    } else {
        KaiType::Int32
    }
}

// -- v0.0.3: calls, field access, struct literals ---------------------------

/// Only direct calls to declared functions exist (§9.3). Functions and types
/// share names freely — namespaces are separate — so a struct name is NOT a
/// valid callee.
fn call_expr(checker: &mut Checker, call: &CallExpr, span: Span) -> TypedExpr {
    let func_id = match &call.callee.kind {
        ExprKind::Ident(ident) => match checker.resolution.fns.get(&ident.name) {
            Some(&idx) => FunctionId(idx as u32),
            None => {
                checker.error(error::unknown_function(&ident.name, ident.span));
                return TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32);
            }
        },
        _ => {
            checker.error(error::indirect_call(span));
            return TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32);
        }
    };

    let sig = checker.fn_signature(func_id);
    if call.args.len() != sig.param_tys.len() {
        let expected = sig.param_tys.len();
        let found = call.args.len();
        checker.error(error::arg_count_mismatch(expected, found, span));
        return TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32);
    }

    let mut args = Vec::with_capacity(call.args.len());
    for (position, (arg, param_ty)) in call.args.iter().zip(&sig.param_tys).enumerate() {
        let value = lower(checker, arg, Some(*param_ty));
        // The hint widens int literals; everything else must match exactly.
        if value.ty != *param_ty {
            checker.error(error::arg_type_mismatch(
                *param_ty,
                value.ty,
                position + 1,
                arg.span,
            ));
        }
        args.push(value);
    }

    TypedExpr::new(
        TypedExprKind::Call {
            func: func_id,
            args,
        },
        sig.ret,
    )
}

/// `base.field`. The base must be a declared struct; the result type is the
/// field's type. Reads COPY out of the place (§9.3).
fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = lower(checker, &access.base, None);

    let struct_id = match base.ty {
        KaiType::Struct(id) => id,
        other => {
            checker.error(error::field_access_on_non_struct(other, access.field.span));
            return TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32);
        }
    };

    match checker.field_slot(struct_id, &access.field.name) {
        Some((index, slot)) => TypedExpr::new(
            TypedExprKind::FieldAccess {
                base: Box::new(base),
                struct_id,
                field: index,
            },
            slot.ty,
        ),
        None => {
            let ty_name = checker.type_name(struct_id).to_string();
            let field = access.field.name.clone();
            checker.error(error::no_such_field(&ty_name, &field, access.field.span));
            TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32)
        }
    }
}

/// `Name { f: e, .. }` — every field exactly once, in any source order; the
/// lowered values are reordered into declaration order (the ABI layout).
fn struct_lit(checker: &mut Checker, lit: &StructLitExpr) -> TypedExpr {
    let struct_id = match checker.resolution.types.get(&lit.name.name) {
        Some(&idx) => StructId(idx as u32),
        None => {
            checker.error(error::unknown_type(&lit.name.name, lit.name.span));
            return TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32);
        }
    };
    let ty_name = checker.type_name(struct_id).to_string();
    let layout_len = checker.structs[struct_id.0 as usize].fields.len();

    // provided[i] = Some(value) once field i has been initialized.
    let mut provided: Vec<Option<TypedExpr>> = vec![None; layout_len];
    let mut seen_dup: Vec<bool> = vec![false; layout_len];

    for init in &lit.fields {
        match checker.field_slot(struct_id, &init.name.name) {
            Some((index, slot)) => {
                let expected_ty = slot.ty;
                if seen_dup[index as usize] {
                    let field = init.name.name.clone();
                    checker.error(error::duplicate_field_init(&field, init.name.span));
                } else {
                    seen_dup[index as usize] = true;
                    let value = lower(checker, &init.value, Some(expected_ty));
                    // The hint widens int literals; everything else must
                    // match the declared field type exactly.
                    if value.ty != expected_ty {
                        let field = init.name.name.clone();
                        checker.error(error::field_type_mismatch(
                            &field,
                            expected_ty,
                            value.ty,
                            init.value.span,
                        ));
                    }
                    provided[index as usize] = Some(value);
                }
            }
            None => {
                let field = init.name.name.clone();
                checker.error(error::no_such_field(&ty_name, &field, init.name.span));
                // Lower anyway so nested errors surface too.
                lower(checker, &init.value, None);
            }
        }
    }

    let mut values = Vec::with_capacity(layout_len);
    for (slot_index, value) in provided.into_iter().enumerate() {
        match value {
            Some(v) => values.push(v),
            None => {
                let field = checker.structs[struct_id.0 as usize].fields[slot_index]
                    .name
                    .clone();
                checker.error(error::missing_field_in_lit(&field, &ty_name, lit.name.span));
                values.push(TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32));
            }
        }
    }

    TypedExpr::new(
        TypedExprKind::StructLit { struct_id, values },
        KaiType::Struct(struct_id),
    )
}
