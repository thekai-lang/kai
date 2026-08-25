#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
use super::*;

/// §5.1.7 + §5.1.3: a `T @local(d)` value may flow into an argument
/// position expecting the BARE inner type `T`. `@local` has zero runtime
/// footprint (pure delegation — no header, no wrapping), so this is a
/// compile-time marker drop on a read/borrow, never a representation
/// change. Without it, every callee in the call graph would be forced to
/// re-annotate its parameters with `@local` just because the caller happens
/// to hold a tracked value — making "cheap" (§5.1) a lie.
///
/// Soundness is NOT lost: the modifier stays on the TAST node (no coercion),
/// and the EFFECT checker runs after ownership with full call-graph effect
/// knowledge — if the callee turns out escaping (declared OR inferred
/// transitively), the boundary diagnostic fires there. Typecheck cannot
/// make that call: effects do not exist yet at this phase (§8 ordering).
pub(crate) fn local_read_as_plain(value_ty: &KaiType, param_ty: &KaiType) -> bool {
    matches!(
        value_ty,
        KaiType::Temporal {
            inner,
            origin: kai_tast::TemporalOrigin::Local,
            ..
        } if inner.as_ref() == param_ty
    )
}

pub(crate) fn call_expr(checker: &mut Checker, call: &CallExpr, span: Span) -> TypedExpr {
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
        // The hint widens int literals; everything else must match exactly —
        // except the @local read-widening above (soundness deferred to the
        // effect checker, which knows this callee's inferred effects).
        if value.ty != *param_ty && !local_read_as_plain(&value.ty, param_ty) {
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

pub(crate) fn qualified_callee(checker: &mut Checker, access: &FieldAccessExpr) -> Option<FunctionId> {
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
        let _ = super::field_access(checker, access);
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

pub(crate) fn try_closure_call(checker: &mut Checker, call: &CallExpr, span: Span) -> Option<TypedExpr> {
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
        if value.ty != *param_ty && !local_read_as_plain(&value.ty, param_ty) {
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

pub(crate) fn unwrap_or_builtin(
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

