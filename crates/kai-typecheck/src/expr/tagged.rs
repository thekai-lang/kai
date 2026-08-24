#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
use super::*;

pub(crate) fn coalesce_expr(checker: &mut Checker, c: &CoalesceExpr) -> TypedExpr {
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

pub(crate) fn catch_expr(checker: &mut Checker, c: &CatchExpr) -> TypedExpr {
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

pub(crate) fn closure_literal(checker: &mut Checker, clo: &kai_ast::ClosureLitExpr) -> TypedExpr {
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
