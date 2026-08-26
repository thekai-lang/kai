#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
//! Expression typing. Rules (§9.2): strict same-type arithmetic, `%` ints
//! only, comparisons yield `bool`, `&&`/`||`/`!` are `bool`-only. Integer
//! literals default to `int32` and widen to `int64` only when context demands
//! it (annotation, return type, or the other operand's concrete type).

use crate::checker::Checker;
use crate::error;
use crate::scope::DeclareOutcome;
use crate::stmt;
use kai_ast::{
    ArrayLitExpr, BinaryExpr, BinaryOp as AstBinaryOp, CallExpr, CatchExpr, CoalesceExpr, Expr,
    ExprKind, FieldAccessExpr, Ident, IndexExpr, StructLitExpr, UnaryOp,
};
use kai_diagnostics::Span;
use kai_tast::{
    BinaryOp, FunctionId, KaiType, LocalId, StructId, TypedCapture, TypedClosure, TypedExpr,
    TypedExprKind,
};

/// Lower an AST expression to TAST. `expected` is a width hint for integer
/// literals; it never enables implicit conversions.
mod call;
mod struct_lit;
mod array;
mod tagged;
mod collect;
pub(crate) use struct_lit::{field_access, resolve_field_hop, resolve_index_hop, struct_lit};
pub(crate) use array::{array_lit, index_expr};
pub(crate) use tagged::{capture_poisoned, catch_expr, closure_literal, coalesce_expr};
pub(crate) use collect::{collect_local_refs, collect_refs_expr, collect_refs_stmt, is_import_alias};
pub(crate) use call::{call_expr, qualified_callee, try_closure_call, unwrap_or_builtin};
pub(crate) fn lower(checker: &mut Checker, expr: &Expr, expected: Option<KaiType>) -> TypedExpr {
    let mut typed = match &expr.kind {
        ExprKind::IntLit(lit) => {
            let span = lit.span;
            int_lit(checker, lit.value, expected.as_ref(), span)
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
        ExprKind::Call(call) => call::call_expr(checker, call, expr.span),
        ExprKind::FieldAccess(access) => struct_lit::field_access(checker, access),
        ExprKind::StructLit(lit) => struct_lit::struct_lit(checker, lit, expr.span),
        ExprKind::ArrayLit(lit) => array::array_lit(checker, lit, expected.as_ref(), expr.span),
        ExprKind::StrLit(lit) => {
            // If expected is `string @local(d)` or `@wallclock(d)`, produce Temporal type for the literal
            // (the literal's creation point is where the temporal duration starts, §5.1).
            if let Some(KaiType::Temporal { inner, origin, duration }) = expected.clone()
                && *inner == KaiType::String
            {
                return TypedExpr::new(
                    TypedExprKind::StrLit {
                        value: lit.value.clone(),
                    },
                    KaiType::Temporal {
                        inner,
                        origin,
                        duration,
                    },
                );
            }
            TypedExpr::new(
                TypedExprKind::StrLit {
                    value: lit.value.clone(),
                },
                KaiType::String,
            )
        }
        ExprKind::Index(indexed) => array::index_expr(checker, indexed),
        // v0.0.6 (§9.9a/§9.10) + v0.14 Ok/Err
        ExprKind::SomeLit(some) => {
            let value = lower(checker, &some.value, None);
            let ty = KaiType::Optional(Box::new(value.ty.clone()));
            TypedExpr::new(TypedExprKind::SomeLit(Box::new(value)), ty)
        }
        ExprKind::NoneLit => {
            // `None` carries no payload to infer from: a context type must
            // fix T, exactly like the empty array literal (§9.7's rule).
            match expected {
                Some(KaiType::Optional(payload)) => {
                    TypedExpr::new(TypedExprKind::NoneLit, KaiType::Optional(payload))
                }
                _ => {
                    checker.error(error::none_needs_annotation(expr.span));
                    poisoned()
                }
            }
        }
        ExprKind::OkLit(ok) => {
            // `Ok(value)` pins T from its arg, E from context (annotation or return type) — v0.14 §3.4.
            let value = lower(checker, &ok.value, None);
            let ok_ty = value.ty.clone();
            match expected {
                Some(KaiType::Result { ok: _, err }) => {
                    let ty = KaiType::Result {
                        ok: Box::new(ok_ty.clone()),
                        err: err.clone(),
                    };
                    TypedExpr::new(TypedExprKind::OkLit(Box::new(value)), ty)
                }
                _ => {
                    checker.error(error::ok_needs_annotation(expr.span));
                    poisoned()
                }
            }
        }
        ExprKind::ErrLit(err_lit) => {
            // `Err(value)` pins E from its arg, T from context.
            let value = lower(checker, &err_lit.value, None);
            let err_ty = value.ty.clone();
            match expected {
                Some(KaiType::Result { ok, err: _ }) => {
                    let ty = KaiType::Result {
                        ok: ok.clone(),
                        err: Box::new(err_ty.clone()),
                    };
                    TypedExpr::new(TypedExprKind::ErrLit(Box::new(value)), ty)
                }
                _ => {
                    checker.error(error::err_needs_annotation(expr.span));
                    poisoned()
                }
            }
        }
        ExprKind::Coalesce(c) => tagged::coalesce_expr(checker, c),
        ExprKind::Catch(c) => tagged::catch_expr(checker, c),
        ExprKind::ClosureLit(clo) => tagged::closure_literal(checker, clo),
        // Poisoned parser-recovery node. The program already failed upstream;
        // this defensive diagnostic keeps the phase contract explicit.
        ExprKind::Invalid => {
            let span = expr.span;
            checker.error(error::invalid_expression(span));
            poisoned()
        }
    };
    // Every node carries its whole-expression span; runtime panic sites
    // (§10.1) resolve it to `file:line:col` at emission time.
    typed.span = expr.span;
    typed
}

fn int_lit(checker: &mut Checker, value: u64, expected: Option<&KaiType>, span: Span) -> TypedExpr {
    // Scalar literals coerce into a temporal wrapper when the expected type
    // is `int32/int64 @local(d)` or `@wallclock(d)` — mirrors string-literal
    // coercion above (creation point starts the duration clock, §5.1).
    if let Some(KaiType::Temporal { inner, origin, duration }) = expected
        && matches!(inner.as_ref(), KaiType::Int32 | KaiType::Int64)
    {
        let scalar = if matches!(inner.as_ref(), KaiType::Int64) {
            KaiType::Int64
        } else {
            KaiType::Int32
        };
        let max_inclusive: u64 = match scalar {
            KaiType::Int32 => i32::MAX as u64,
            _ => i64::MAX as u64,
        };
        if value > max_inclusive {
            checker.error(error::literal_out_of_range(max_inclusive, scalar.clone(), span));
        }
        return TypedExpr::new(
            TypedExprKind::IntLit(value as i64),
            KaiType::Temporal {
                inner: inner.clone(),
                origin: origin.clone(),
                duration: duration.clone(),
            },
        );
    }
    let ty = if expected == Some(&KaiType::Int64) {
        KaiType::Int64
    } else {
        KaiType::Int32
    };
    let max_inclusive: u64 = match ty {
        KaiType::Int32 => i32::MAX as u64,
        _ => i64::MAX as u64,
    };
    if value > max_inclusive {
        checker.error(error::literal_out_of_range(max_inclusive, ty.clone(), span));
    }
    TypedExpr::int_lit(value as i64, ty)
}

fn ident_ref(checker: &mut Checker, ident: &Ident) -> TypedExpr {
    match checker.locals.lookup(&ident.name) {
        Some(info) => TypedExpr::new(TypedExprKind::LocalRef(info.id), info.ty.clone()),
        None => {
            let span = ident.span;
            let name = ident.name.clone();
            checker.error(error::undeclared_variable(&name, span));
            // Placeholder keeps compilation going; program is discarded.
            zero_int()
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
            let ty = inner.ty.clone();
            if ty.is_numeric() {
                TypedExpr::new(TypedExprKind::Neg(Box::new(inner)), ty)
            } else {
                let span = operand.span;
                checker.error(error::operand_type_mismatch("-", ty, span));
                zero_int()
            }
        }
        UnaryOp::Not => {
            let inner = lower(checker, operand, None);
            if inner.ty == KaiType::Bool {
                TypedExpr::new(TypedExprKind::Not(Box::new(inner)), KaiType::Bool)
            } else {
                let span = operand.span;
                let ty = inner.ty.clone();
                checker.error(error::operand_type_mismatch("!", ty, span));
                TypedExpr::new(TypedExprKind::BoolLit(false), KaiType::Bool)
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
            checker.error(error::literal_out_of_range(i64::MAX as u64, ty.clone(), span));
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
        ExprKind::IntLit(_) => Some(lhs.ty.clone()).filter(|ty| ty.is_integer()),
        _ => None,
    };
    let rhs = lower(checker, &binary.rhs, rhs_hint);

    typed_binary(checker, op, lhs, rhs, span)
}

/// Every binary typing rule shares one shape: validate the operand pair
/// against the operator's class, report the class's diagnostic, then build
/// the node with the class's result type (the operand type, or `bool`).
/// Rules that fail keep compiling with placeholders — the program is
/// discarded anyway.
fn typed_binary(
    checker: &mut Checker,
    op: BinaryOp,
    lhs: TypedExpr,
    rhs: TypedExpr,
    span: Span,
) -> TypedExpr {
    let lty = lhs.ty.clone();
    let rty = rhs.ty.clone();
    let name = op.describe();

    match op {
        // `+ - * /`: one shared numeric type; the result keeps it.
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
            if !lty.is_numeric() || lty != rty {
                checker.error(if lty.is_numeric() && rty.is_numeric() {
                    error::binary_type_mismatch(name, lty.clone(), rty.clone(), span)
                } else {
                    let bad = if lty.is_numeric() { rty } else { lty };
                    error::operand_type_mismatch(name, bad, span)
                });
                return zero_int();
            }
            binary_node(op, lhs, rhs, lty)
        }
        // `%`: integers only; the failed-rule result still prefers an
        // integer lhs so downstream rules see a sane shape.
        BinaryOp::Mod => {
            if lty != rty || !lty.is_integer() {
                checker.error(error::mod_requires_integers(span));
            }
            let result_ty = lhs_placeholder_ty(lty);
            binary_node(op, lhs, rhs, result_ty)
        }
        // `< > <= >=` compare numerics pairwise; `== !=` accept any ONE
        // shared type (strings included). Both produce `bool`.
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            if !lty.is_numeric() || lty != rty {
                checker.error(error::binary_type_mismatch(name, lty.clone(), rty.clone(), span));
            }
            binary_node(op, lhs, rhs, KaiType::Bool)
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            if lty != rty {
                checker.error(error::binary_type_mismatch(name, lty.clone(), rty.clone(), span));
            }
            binary_node(op, lhs, rhs, KaiType::Bool)
        }
        // `&& ||` are bool-only; point at whichever operand strayed.
        BinaryOp::And | BinaryOp::Or => {
            if lty != KaiType::Bool || rty != KaiType::Bool {
                let bad = if lty == KaiType::Bool { rty } else { lty };
                checker.error(error::operand_type_mismatch(name, bad, span));
            }
            binary_node(op, lhs, rhs, KaiType::Bool)
        }
    }
}

fn binary_node(op: BinaryOp, lhs: TypedExpr, rhs: TypedExpr, ty: KaiType) -> TypedExpr {
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty,
    )
}

/// Placeholder int for expressions whose real type is unknowable after an
/// operand-class error; keeps numeric contexts well-typed while the doomed
/// program finishes lowering.
fn zero_int() -> TypedExpr {
    TypedExpr::new(TypedExprKind::IntLit(0), KaiType::Int32)
}

/// Placeholder for expressions that cannot be resolved at all (unknown
/// callee, missing field, parse recovery). Never reaches codegen: any
/// diagnostic fails the phase before emission.
fn poisoned() -> TypedExpr {
    TypedExpr::new(TypedExprKind::Invalid, KaiType::Int32)
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

