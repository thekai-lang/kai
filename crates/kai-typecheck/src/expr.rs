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
        ExprKind::Call(call) => call_expr(checker, call, expr.span),
        ExprKind::FieldAccess(access) => field_access(checker, access),
        ExprKind::StructLit(lit) => struct_lit(checker, lit, expr.span),
        ExprKind::ArrayLit(lit) => array_lit(checker, lit, expected.as_ref(), expr.span),
        ExprKind::StrLit(lit) => TypedExpr::new(
            TypedExprKind::StrLit {
                value: lit.value.clone(),
            },
            KaiType::String,
        ),
        ExprKind::Index(indexed) => index_expr(checker, indexed),
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
        ExprKind::Coalesce(c) => coalesce_expr(checker, c),
        ExprKind::Catch(c) => catch_expr(checker, c),
        ExprKind::ClosureLit(clo) => closure_literal(checker, clo),
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

/// Only two callee shapes exist (§9.3): a plain name resolved inside the
/// current module, or `alias.member` naming a PUBLIC function of an imported
/// module. Functions and types share names freely — namespaces are separate
/// — so a struct name is NOT a valid callee.
fn call_expr(checker: &mut Checker, call: &CallExpr, span: Span) -> TypedExpr {
    // `.unwrap_or(default)` on a tagged-union receiver is the builtin
    // combinator (§9.9a) — resolved HERE, from an ordinary FieldAccess+Call
    // shape, exactly as the grammar note promises. An import alias named
    // `unwrap_or` still wins: module calls keep their resolution path.
    if let ExprKind::FieldAccess(access) = &call.callee.kind
        && access.field.name == "unwrap_or"
        && !is_import_alias(checker, &access.base)
    {
        return unwrap_or_builtin(checker, access, call, span);
    }

    let func_id = match &call.callee.kind {
        ExprKind::Ident(ident) => match checker.local_fns().get(&ident.name) {
            Some(&idx) => FunctionId(idx as u32),
            None => {
                // Not a declared function — maybe a closure-VALUED local
                // (v0.0.6 first-class calls).
                if checker
                    .locals
                    .lookup(&ident.name)
                    .is_some_and(|info| matches!(info.ty, KaiType::Closure { .. }))
                    && let Some(t) = try_closure_call(checker, call, span)
                {
                    return t;
                }
                checker.error(error::unknown_function(&ident.name, ident.span));
                return poisoned();
            }
        },
        ExprKind::FieldAccess(access) => {
            if !is_import_alias(checker, &access.base)
                && access.field.name != "unwrap_or"
            {
                // Maybe a closure stored in a field / reached by projection.
                if let Some(t) = try_closure_call(checker, call, span) {
                    return t;
                }
            }
            match qualified_callee(checker, access) {
                Some(id) => id,
                None => return poisoned(),
            }
        }
        _ => {
            // Not a named function: maybe a closure VALUE. Typing supports
            // the call; emission goes indirect (P5).
            if let Some(t) = try_closure_call(checker, call, span) {
                return t;
            }
            checker.error(error::indirect_call(span));
            return poisoned();
        }
    };

    let sig = checker.fn_signature(func_id);
    if call.args.len() != sig.param_tys.len() {
        let expected = sig.param_tys.len();
        let found = call.args.len();
        checker.error(error::arg_count_mismatch(&sig.name, expected, found, span));
        return poisoned();
    }

    let mut args = Vec::with_capacity(call.args.len());
    for (position, (arg, param_ty)) in call.args.iter().zip(&sig.param_tys).enumerate() {
        let value = lower(checker, arg, Some(param_ty.clone()));
        // The hint widens int literals; everything else must match exactly.
        if value.ty != *param_ty {
            checker.error(error::arg_type_mismatch(
                &sig.name,
                param_ty.clone(),
                value.ty.clone(),
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

/// `alias.member(...)`: the head must name an import of the current module;
/// the member must be a PUBLIC function of the target module. Anything else
/// is not a module call — the base is then treated as an ordinary value
/// (two-branch rule, §9.3): field access lowers normally and calling its
/// result is rejected.
fn qualified_callee(checker: &mut Checker, access: &FieldAccessExpr) -> Option<FunctionId> {
    let alias = match &access.base.kind {
        ExprKind::Ident(ident) => ident,
        _ => {
            checker.error(error::indirect_call(access.base.span));
            return None;
        }
    };

    let Some(&target) = checker.imports().get(&alias.name) else {
        // Not an import alias: ordinary value semantics. Lower the field
        // access for its diagnostics, then reject the call itself.
        let _ = field_access(checker, access);
        checker.error(error::indirect_call(access.base.span));
        return None;
    };

    let path = format!("{}.{}", alias.name, access.field.name);
    let Some(&idx) = checker.resolution.module_fns[target].get(&access.field.name)
    else {
        checker.error(error::unknown_qualified_function(&path, access.field.span));
        return None;
    };
    if !checker.resolution.fn_is_public[idx] {
        checker.error(error::private_function(&path, access.field.span));
        return None;
    }
    Some(FunctionId(idx as u32))
}

/// One `.` hop resolved against a known type — the shared core of
/// expression field access and assignment-place walking, so both report
/// identical errors keyed to the segment's span.
pub(crate) fn resolve_field_hop(
    checker: &mut Checker,
    cur: &KaiType,
    field: &str,
    span: Span,
) -> Option<(StructId, u16, KaiType)> {
    let struct_id = match cur {
        KaiType::Struct(id) => *id,
        other => {
            checker.error(error::field_access_on_non_struct(other.clone(), span));
            return None;
        }
    };
    match checker.field_slot(struct_id, field) {
        Some((index, slot)) => Some((struct_id, index, slot.ty.clone())),
        None => {
            let ty_name = checker.type_name(struct_id).to_string();
            checker.error(error::no_such_field(&ty_name, field, span));
            None
        }
    }
}

/// One `[..]` hop: array-shape check plus a lowered, integer-checked index
/// expression (§9.3).
pub(crate) fn resolve_index_hop(
    checker: &mut Checker,
    cur: &KaiType,
    index: &Expr,
    rbracket: Span,
) -> Option<(KaiType, TypedExpr)> {
    let elem_ty = match cur {
        KaiType::Array(elem) => elem.as_ref().clone(),
        other => {
            checker.error(error::index_on_non_array(other, rbracket));
            return None;
        }
    };
    let typed_index = lower(checker, index, None);
    if !typed_index.ty.is_integer() {
        let ty = typed_index.ty.clone();
        checker.error(error::index_not_integer(&ty, rbracket));
    }
    Some((elem_ty, typed_index))
}

/// `base.field`. The base must be a declared struct; the result type is the
/// field's type. Reads COPY out of the place (§9.3).
fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = lower(checker, &access.base, None);
    let Some((struct_id, field, ty)) =
        resolve_field_hop(checker, &base.ty, &access.field.name, access.field.span)
    else {
        return poisoned();
    };
    TypedExpr::new(
        TypedExprKind::FieldAccess {
            base: Box::new(base),
            struct_id,
            field,
        },
        ty,
    )
}

/// `Name { f: e, .. }` — every field exactly once, in any source order; the
/// lowered values are reordered into declaration order (the ABI layout).
/// The head is either an unqualified name (own module) or
/// `alias.Name` naming a PUBLIC struct of an imported module.
fn struct_lit(checker: &mut Checker, lit: &StructLitExpr, lit_span: Span) -> TypedExpr {
    let segments = lit.path.len();
    let struct_id = if segments == 1 {
        // Unqualified: own module only.
        let type_name = &lit.path[0];
        match checker.local_types().get(&type_name.name) {
            Some(&idx) => StructId(idx as u32),
            None => {
                checker.error(error::unknown_type(&type_name.name, type_name.span));
                return poisoned();
            }
        }
    } else {
        // Qualified head: first segment must be an import alias.
        let alias = &lit.path[0];
        let member = lit.path.last().expect("non-empty literal head");
        let path = format!(
            "{}.{}",
            alias.name,
            lit.path[1..]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".")
        );
        match checker.imports().get(&alias.name) {
            Some(&target) => {
                let Some(&idx) = checker.resolution.module_types[target].get(&member.name)
                else {
                    checker.error(error::unknown_qualified_type(&path, member.span));
                    return poisoned();
                };
                if !checker.resolution.type_is_public[idx] {
                    checker.error(error::private_type(&path, member.span));
                    return poisoned();
                }
                StructId(idx as u32)
            }
            None => {
                checker.error(error::unknown_module(&alias.name, alias.span));
                return poisoned();
            }
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
                let expected_ty = slot.ty.clone();
                if seen_dup[index as usize] {
                    let field = init.name.name.clone();
                    checker.error(error::duplicate_field_init(&field, init.name.span));
                } else {
                    seen_dup[index as usize] = true;
                    let value = lower(checker, &init.value, Some(expected_ty.clone()));
                    // The hint widens int literals; everything else must
                    // match the declared field type exactly.
                    if value.ty != expected_ty {
                        let field = init.name.name.clone();
                        checker.error(error::field_type_mismatch(
                            &field,
                            expected_ty.clone(),
                            value.ty.clone(),
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
                checker.error(error::missing_field_in_lit(&field, &ty_name, lit_span));
                values.push(poisoned());
            }
        }
    }

    TypedExpr::new(
        TypedExprKind::StructLit { struct_id, values },
        KaiType::Struct(struct_id),
    )
}

// -- v0.0.5: strings, arrays, indexing ---------------------------------------

/// `[e0, e1, ..]`: every element unifies to ONE type; the context hint (an
/// expected `T[]`) types bare int literals and — decisively — makes an
/// EMPTY literal legal. `let a = [];` with no annotation is an error
/// (§9.7): there is nothing to infer from.
fn array_lit(
    checker: &mut Checker,
    lit: &ArrayLitExpr,
    expected: Option<&KaiType>,
    lit_span: Span,
) -> TypedExpr {
    let elem_hint: Option<KaiType> = match expected {
        Some(KaiType::Array(elem)) => Some(elem.as_ref().clone()),
        _ => None,
    };

    if lit.elements.is_empty() {
        return match elem_hint {
            Some(elem) => TypedExpr::new(TypedExprKind::ArrayLit { elements: vec![] }, KaiType::Array(Box::new(elem))),
            None => {
                checker.error(error::empty_array_needs_annotation(lit_span));
                poisoned()
            }
        };
    }

    let mut typed: Vec<TypedExpr> = Vec::with_capacity(lit.elements.len());
    let mut elem_ty: Option<KaiType> = None;
    for element in &lit.elements {
        let value = lower(checker, element, elem_hint.clone());
        match &elem_ty {
            None => elem_ty = Some(value.ty.clone()),
            Some(expected_ty) => {
                if value.ty != *expected_ty {
                    let found = value.ty.clone();
                    checker.error(error::array_element_mismatch(expected_ty, found, element.span));
                }
            }
        }
        typed.push(value);
    }
    let elem = elem_ty.unwrap_or(KaiType::Int32);
    TypedExpr::new(
        TypedExprKind::ArrayLit { elements: typed },
        KaiType::Array(Box::new(elem)),
    )
}

/// `base[index]`: base must be `T[]`, index any integer width; the result is
/// a plain read of `T` (§9.3). Bounds are checked at RUNTIME in later
/// releases; here we only guarantee the shapes line up.
fn index_expr(checker: &mut Checker, indexed: &IndexExpr) -> TypedExpr {
    let base = lower(checker, &indexed.base, None);
    let Some((elem_ty, index)) =
        resolve_index_hop(checker, &base.ty, &indexed.index, indexed.rbracket)
    else {
        return poisoned();
    };
    TypedExpr::new(
        TypedExprKind::Index {
            base: Box::new(base),
            index: Box::new(index),
        },
        elem_ty,
    )
}

// -- v0.0.6 (§9.9a/§9.10) -------------------------------------------------------

/// `lhs ?? rhs` — lhs must be `Optional<T>`; the fallback unifies with the
/// payload `T`, which is also the result type. Laziness is a lowering
/// concern; typing only fixes the shapes.
fn coalesce_expr(checker: &mut Checker, c: &CoalesceExpr) -> TypedExpr {
    let lhs = lower(checker, &c.lhs, None);
    let payload = match lhs.ty.clone() {
        KaiType::Optional(t) => *t,
        other => {
            checker.error(error::coalesce_on_non_optional(other, c.lhs.span));
            return poisoned();
        }
    };
    let rhs = lower(checker, &c.rhs, Some(payload.clone()));
    if rhs.ty != payload {
        checker.error(error::coalesce_default_mismatch(
            payload.clone(),
            rhs.ty.clone(),
            c.rhs.span,
        ));
    }
    TypedExpr::new(
        TypedExprKind::Coalesce {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        payload,
    )
}

/// `base catch |err| { stmts.. tail }` — base must be `Result<T, E>`; the
/// binding views `E` (a borrow of the Err payload), the block must produce
/// `T`. The binding lives in its own scope for exactly the catch block.
fn catch_expr(checker: &mut Checker, c: &CatchExpr) -> TypedExpr {
    let base = lower(checker, &c.base, None);
    let (ok_ty, err_ty) = match base.ty.clone() {
        KaiType::Result { ok, err } => (*ok, *err),
        other => {
            checker.error(error::catch_on_non_result(other, c.base.span));
            return poisoned();
        }
    };

    checker.locals.push_scope();
    // Duplicate-in-scope is impossible in a fresh scope; shadowing an outer
    // name is ordinary scoping and allowed.
    checker
        .locals
        .declare(&c.err_binding.name, err_ty.clone(), false);
    let Some(err_info) = checker.locals.lookup(&c.err_binding.name) else {
        unreachable!("just declared")
    };
    let ret_hint = ok_ty.clone();
    let stmts: Vec<_> = c
        .stmts
        .iter()
        .filter_map(|s| stmt::lower_stmt(checker, s, &ret_hint))
        .collect();
    let tail = lower(checker, &c.tail, Some(ok_ty.clone()));
    checker.locals.pop_scope();

    if tail.ty != ok_ty {
        checker.error(error::catch_tail_mismatch(
            ok_ty.clone(),
            tail.ty.clone(),
            c.tail.span,
        ));
    }
    TypedExpr::new(
        TypedExprKind::Catch {
            base: Box::new(base),
            err_binding: err_info.id,
            err_ty,
            stmts,
            tail: Box::new(tail),
            releases: Vec::new(),
        },
        ok_ty,
    )
}

fn closure_literal(checker: &mut Checker, clo: &kai_ast::ClosureLitExpr) -> TypedExpr {
    let boundary = checker.locals.next_id();

    checker.locals.push_scope();
    let mut params_typed = Vec::with_capacity(clo.params.len());
    for param in &clo.params {
        let ty = crate::ty::resolve(checker, &param.ty);
        if let DeclareOutcome::Fresh(info) =
            checker.locals.declare(&param.name.name, ty, param.mutable)
        {
            params_typed.push((info.id, info.ty.clone()));
        }
    }
    let ret = crate::ty::resolve(checker, &clo.ret);
    let body = stmt::lower_block(checker, &clo.body, &ret);
    checker.locals.pop_scope();

    if ret != KaiType::Unit && !crate::decl::definitely_returns(&body) {
        checker.error(error::closure_needs_return(ret.clone(), clo.body.span));
    }

    // Capture analysis over the TYPED body: LocalRef ids below the boundary
    // point outside this scope. Nested closures manage their own captures.
    let mut refs = Vec::new();
    collect_local_refs(&body, &mut refs);
    let mut captures: Vec<TypedCapture> = Vec::new();
    for id in refs {
        if id.0 >= boundary || captures.iter().any(|c| c.local == id) {
            continue;
        }
        let Some(info) = checker.locals.info_of(id) else {
            continue;
        };
        if capture_poisoned(checker, &info.ty) {
            checker.error(error::closure_capture_banned(
                &checker.locals.name_of(id),
                &info.ty,
                clo.body.span,
            ));
            break;
        }
        captures.push(TypedCapture {
            local: id,
            ty: info.ty.clone(),
        });
    }

    let ty = KaiType::Closure {
        params: params_typed.iter().map(|(_, t)| t.clone()).collect(),
        ret: Box::new(ret),
    };
    TypedExpr::new(
        TypedExprKind::ClosureLit(Box::new(TypedClosure {
            param_ids: params_typed.into_iter().map(|(id, _)| id).collect(),
            body,
            captures,
        })),
        ty,
    )
}

/// Does this type contain a closure type through any member path? Structs
/// consult the resolver's §9.10 poisoning table (transitively precomputed).
pub(crate) fn capture_poisoned(checker: &Checker, ty: &KaiType) -> bool {
    match ty {
        KaiType::Closure { .. } => true,
        KaiType::Array(elem) => capture_poisoned(checker, elem),
        KaiType::Optional(inner) => capture_poisoned(checker, inner),
        KaiType::Result { ok, err } => {
            capture_poisoned(checker, ok) || capture_poisoned(checker, err)
        }
        KaiType::Struct(id) => checker
            .resolution
            .closure_bearing
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false),
        _ => false,
    }
}

/// First-use-order LocalRef collection. Nested closures are NOT descended:
/// their bodies belong to their own environments.
fn collect_local_refs(block: &kai_tast::TypedBlock, out: &mut Vec<LocalId>) {
    for s in &block.stmts {
        collect_refs_stmt(s, out);
    }
}

fn collect_refs_stmt(s: &kai_tast::TypedStmt, out: &mut Vec<LocalId>) {
    use kai_tast::TypedStmt;
    match s {
        TypedStmt::Let(l) => collect_refs_expr(&l.init, out),
        TypedStmt::Assign(a) => {
            for step in &a.path {
                if let kai_tast::TypedPlaceStep::Index(index) = step {
                    collect_refs_expr(index, out);
                }
            }
            collect_refs_expr(&a.value, out);
        }
        TypedStmt::Return(Some(e)) => collect_refs_expr(e, out),
        TypedStmt::If(i) => {
            collect_refs_expr(&i.cond, out);
            collect_local_refs(&i.then_block, out);
            if let Some(e) = &i.else_block {
                collect_local_refs(e, out);
            }
        }
        TypedStmt::For(f) => {
            collect_refs_expr(&f.iterable, out);
            collect_local_refs(&f.body, out);
        }
        TypedStmt::Block(b) => collect_local_refs(b, out),
        TypedStmt::Expr(e) => collect_refs_expr(e, out),
        // Ownership-pass markers: no user references inside.
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {}
        TypedStmt::Return(None) => {}
    }
}

fn collect_refs_expr(e: &TypedExpr, out: &mut Vec<LocalId>) {
    match &e.kind {
        TypedExprKind::LocalRef(id) => {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        TypedExprKind::Neg(inner)
        | TypedExprKind::Not(inner)
        | TypedExprKind::Retain(inner)
        | TypedExprKind::SomeLit(inner)
        | TypedExprKind::OkLit(inner)
        | TypedExprKind::ErrLit(inner) => collect_refs_expr(inner, out),
        TypedExprKind::Binary { lhs, rhs, .. }
        | TypedExprKind::Coalesce { lhs, rhs } => {
            collect_refs_expr(lhs, out);
            collect_refs_expr(rhs, out);
        }
        TypedExprKind::FieldAccess { base, .. } => collect_refs_expr(base, out),
        TypedExprKind::UnwrapOr { receiver, default } => {
            collect_refs_expr(receiver, out);
            collect_refs_expr(default, out);
        }
        TypedExprKind::Index { base, index } => {
            collect_refs_expr(base, out);
            collect_refs_expr(index, out);
        }
        TypedExprKind::StructLit { values, .. } | TypedExprKind::ArrayLit { elements: values } => {
            for v in values {
                collect_refs_expr(v, out);
            }
        }
        TypedExprKind::Call { args, .. } => {
            for a in args {
                collect_refs_expr(a, out);
            }
        }
        TypedExprKind::CallIndirect { callee, args } => {
            collect_refs_expr(callee, out);
            for a in args {
                collect_refs_expr(a, out);
            }
        }
        TypedExprKind::Catch {
            base, stmts, tail, ..
        } => {
            collect_refs_expr(base, out);
            for s in stmts {
                collect_refs_stmt(s, out);
            }
            collect_refs_expr(tail, out);
        }
        // Nested closures own their captures; literals carry nothing.
        TypedExprKind::ClosureLit(_) | TypedExprKind::NoneLit => {}
        TypedExprKind::IntLit(_) | TypedExprKind::FloatLit(_) | TypedExprKind::BoolLit(_)
        | TypedExprKind::StrLit { .. } | TypedExprKind::Invalid => {}
    }
}

/// True when this expression is a bare identifier naming an IMPORT ALIAS in
/// the current module — such bases go through qualified-call resolution,
/// never the builtin path.
fn is_import_alias(checker: &Checker, base: &Expr) -> bool {
    match &base.kind {
        ExprKind::Ident(ident) => checker.imports().contains_key(&ident.name),
        _ => false,
    }
}

/// Types `f(args)` where `f` evaluates to a closure VALUE. Returns `None`
/// when the callee's type is not a closure (caller then reports its own
/// diagnostic). Argument/result unification follows the signature exactly.
fn try_closure_call(checker: &mut Checker, call: &CallExpr, span: Span) -> Option<TypedExpr> {
    let callee_val = lower(checker, &call.callee, None);
    let KaiType::Closure { params, ret } = callee_val.ty.clone() else {
        return None;
    };
    if call.args.len() != params.len() {
        checker.error(error::arg_count_mismatch(
            &callee_val.ty.to_string(),
            params.len(),
            call.args.len(),
            span,
        ));
        return Some(poisoned());
    }
    let mut args_typed = Vec::with_capacity(call.args.len());
    for (position, (arg, param_ty)) in call.args.iter().zip(&params).enumerate() {
        let value = lower(checker, arg, Some(param_ty.clone()));
        if value.ty != *param_ty {
            checker.error(error::arg_type_mismatch(
                &callee_val.ty.to_string(),
                param_ty.clone(),
                value.ty.clone(),
                position + 1,
                arg.span,
            ));
        }
        args_typed.push(value);
    }
    Some(TypedExpr::new_at(
        TypedExprKind::CallIndirect {
            callee: Box::new(callee_val),
            args: args_typed,
        },
        *ret,
        span,
    ))
}

fn unwrap_or_builtin(
    checker: &mut Checker,
    access: &FieldAccessExpr,
    call: &CallExpr,
    span: Span,
) -> TypedExpr {
    if call.args.len() != 1 {
        checker.error(error::unwrap_or_arity(call.args.len(), span));
        return poisoned();
    }
    let receiver = lower(checker, &access.base, None);
    let want = match receiver.ty.clone() {
        KaiType::Optional(t) => *t,
        KaiType::Result { ok, .. } => *ok,
        other => {
            checker.error(error::unwrap_or_receiver(other, access.base.span));
            return poisoned();
        }
    };
    let default = lower(checker, &call.args[0], Some(want.clone()));
    if default.ty != want {
        checker.error(error::unwrap_or_default_mismatch(
            want.clone(),
            default.ty.clone(),
            call.args[0].span,
        ));
    }
    TypedExpr::new(
        TypedExprKind::UnwrapOr {
            receiver: Box::new(receiver),
            default: Box::new(default),
        },
        want,
    )
}
