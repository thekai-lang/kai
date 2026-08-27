//! Scope-exit release ordering (§9.4) and for..in ownership (§9.9).

use super::tests::{block, fn_decl, int_lit, let_, local_ref, ret, run, str_lit};
use super::*;


#[test]
fn heap_locals_release_at_block_end_reverse_order() {
    let body = block(vec![
        let_(0, "a", str_lit("a")),
        let_(1, "b", str_lit("b")),
        let_(2, "n", int_lit(0)), // scalar: not tracked
        ret(None),
    ]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // stmts: let a, let b, let n, return-with-cleanup(b, a)
    let TypedStmt::ReturnCleanup { releases, .. } = &out.fns[0].body.stmts[3]
    else {
        panic!("expected return cleanup");
    };
    // Reverse declaration order; the scalar `n` and params never appear.
    assert_eq!(
        releases,
        &vec![(LocalId(1), KaiType::String), (LocalId(0), KaiType::String)]
    );
}

#[test]
fn return_inside_nested_block_releases_all_frames() {
    // let a = "..."; if c { return; }   — return must release `a` too.
    let cond = TypedExpr::new(TypedExprKind::BoolLit(true), KaiType::Bool);
    let body = block(vec![
        let_(0, "a", str_lit("a")),
        TypedStmt::If(kai_tast::TypedIf {
            cond,
            then_block: block(vec![ret(None)]),
            else_block: None,
        }),
    ]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // Outer block: [let a, If, release a] — If sits at index 1.
    let TypedStmt::If(if_) = &out.fns[0].body.stmts[1] else { panic!() };
    // then-block carries the OUTER frame's locals in its cleanup:
    // returning from inside the branch still unwinds `a`.
    let TypedStmt::ReturnCleanup { releases, .. } = &if_.then_block.stmts[0]
    else {
        panic!("expected cleanup-carrying return");
    };
    assert_eq!(releases, &vec![(LocalId(0), KaiType::String)]);
    // Normal end of the OUTER block also releases `a` (the branch may
    // fall through): [let a, If, release a].
    assert!(matches!(
        out.fns[0].body.stmts[2],
        TypedStmt::ReleaseLocal { local: LocalId(0), .. }
    ));
}

// ---------- for..in (§9.9 / E7) ----------

#[test]
fn for_over_owned_temp_takes_ownership() {
    let iter = TypedExpr::new(
        TypedExprKind::ArrayLit {
            elements: vec![int_lit(1)],
        },
        KaiType::Array(Box::new(KaiType::Int32)),
    );
    let f = TypedFor {
        binding_local: LocalId(10),
        binding_name: "v".into(),
        iterable: iter,
        body: block(vec![]),
        iterable_owned: false,
    };
    let body = block(vec![TypedStmt::For(f), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // The temp is bound to a hidden local before the loop; the loop now
    // iterates the LOCAL and the flag path is retired.
    assert!(matches!(
        &out.fns[0].body.stmts[0],
        TypedStmt::Let(b) if b.name == "$iter"
    ));
    let TypedStmt::For(f) = &out.fns[0].body.stmts[1] else { panic!() };
    assert!(!f.iterable_owned);
    let iter_local = match &f.iterable.kind {
        TypedExprKind::LocalRef(id) => *id,
        other => panic!("iterable not materialized: {other:?}"),
    };
    // Normal completion: the loop frame pops right after the For —
    // the hidden owner is released there; the loop binding never owns.
    let stmts = &out.fns[0].body.stmts;
    assert!(matches!(
        stmts.get(2),
        Some(TypedStmt::ReleaseLocal { local, .. }) if *local == iter_local
    ));
    assert!(stmts
        .iter()
        .all(|s| !matches!(s, TypedStmt::ReleaseLocal { local: LocalId(10), .. })));
}

#[test]
fn return_inside_loop_releases_owned_iterable() {
    // B3: `for x in [1] { return; }` used to skip the loop-end release.
    let iter = TypedExpr::new(
        TypedExprKind::ArrayLit {
            elements: vec![int_lit(1)],
        },
        KaiType::Array(Box::new(KaiType::Int32)),
    );
    let f = TypedFor {
        binding_local: LocalId(10),
        binding_name: "v".into(),
        iterable: iter,
        body: block(vec![ret(None)]),
        iterable_owned: false,
    };
    let body = block(vec![TypedStmt::For(f)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // The return inside the body must carry the hidden iterable's
    // release alongside the (empty) body frame.
    let mut found_iter_release = false;
    for s in &out.fns[0].body.stmts {
        if let TypedStmt::For(f) = s {
            for inner in &f.body.stmts {
                if let TypedStmt::ReturnCleanup { releases, .. } = inner {
                    found_iter_release =
                        releases.iter().any(|(_, ty)| matches!(ty, KaiType::Array(_)));
                }
            }
        }
    }
    assert!(found_iter_release, "return skips iterable release:\n{out:#?}");
}

#[test]
fn discarded_heap_temp_is_bound_and_released() {
    // B1: `make();` as a statement must not leak the returned string.
    let call = TypedExpr::new(
        TypedExprKind::Call { func: kai_tast::FunctionId(0), args: vec![] },
        KaiType::String,
    );
    let body = block(vec![TypedStmt::Expr(call), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    assert!(matches!(
        &out.fns[0].body.stmts[0],
        TypedStmt::Let(b) if b.name == "$tmp" && b.init.ty == KaiType::String
    ));
    // The following `return` carries the hidden local's release.
    assert!(matches!(
        out.fns[0].body.stmts.last(),
        Some(TypedStmt::ReturnCleanup { releases, .. }) if !releases.is_empty()
    ));
}

#[test]
fn call_arg_temp_is_materialized_in_order() {
    // B1: greet("x") — the literal moves into a hidden local BEFORE the
    // call, the argument becomes a plain borrow of that local.
    let greet = TypedExpr::new(
        TypedExprKind::Call {
            func: kai_tast::FunctionId(1),
            args: vec![str_lit("x")],
        },
        KaiType::Unit,
    );
    let body = block(vec![TypedStmt::Expr(greet), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    let TypedStmt::Let(hidden) = &out.fns[0].body.stmts[0] else { panic!("no hoist:\n{out:#?}") };
    assert_eq!(hidden.name, "$tmp");
    let TypedStmt::Expr(e) = &out.fns[0].body.stmts[1] else { panic!() };
    let TypedExprKind::Call { args, .. } = &e.kind else { panic!() };
    assert!(matches!(args[0].kind, TypedExprKind::LocalRef(_)));
}

#[test]
fn for_over_borrowed_iterable_leaves_ownership_alone() {
    let iter = local_ref(0, KaiType::Array(Box::new(KaiType::Int32)));
    let f = TypedFor {
        binding_local: LocalId(10),
        binding_name: "v".into(),
        iterable: iter,
        body: block(vec![]),
        iterable_owned: false,
    };
    let body = block(vec![
        let_(0, "arr", TypedExpr::new(
            TypedExprKind::ArrayLit { elements: vec![int_lit(1)] },
            KaiType::Array(Box::new(KaiType::Int32)),
        )),
        TypedStmt::For(f),
        ret(None),
    ]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    let TypedStmt::For(f) = &out.fns[0].body.stmts[1] else { panic!() };
    assert!(!f.iterable_owned);
}

// ---------- nested owned-temp hoisting (v0.0.8.4+ hoist_children restructure) ----------

#[test]
fn heap_returning_call_hoists_strlit_arg() {
    // Regression: when the Call ITSELF is heap-bearing (e.g. returns String),
    // the old code hoisted the Call but NOT its StrLit arg — orphaning the
    // creation claim.  With the hoist_children restructure, children are
    // recursed into first, so both the StrLit arg AND the Call are materialized.
    //
    // Note: walk_stmt for a discarded heap Expr skips the Expr node entirely
    // (line 185-190 of walk.rs) — the expression IS the hidden local.
    let call = TypedExpr::new(
        TypedExprKind::Call {
            func: kai_tast::FunctionId(1),
            args: vec![str_lit("x")],
        },
        KaiType::String,
    );
    let body = block(vec![TypedStmt::Expr(call), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // stmts[0]: $tmp0 = "x"  (StrLit arg materialized)
    let TypedStmt::Let(h0) = &out.fns[0].body.stmts[0]
    else { panic!("expected hoisted StrLit arg:\n{out:#?}") };
    assert_eq!(h0.name, "$tmp");
    assert!(
        matches!(h0.init.kind, TypedExprKind::StrLit { .. }),
        "first hoist must be the StrLit arg, got: {:?}", h0.init.kind
    );
    // stmts[1]: $tmp1 = Call(func:1, [LocalRef($tmp0)])
    let TypedStmt::Let(h1) = &out.fns[0].body.stmts[1]
    else { panic!("expected hoisted Call:\n{out:#?}") };
    assert_eq!(h1.name, "$tmp");
    assert!(
        matches!(h1.init.kind, TypedExprKind::Call { .. }),
        "second hoist must be the Call, got: {:?}", h1.init.kind
    );
    // No Expr statement — the Call was a discarded heap temp, so
    // walk_stmt binds it directly without an Expr wrapper.
    // stmts[2] is the ReturnCleanup from ret(None).
    assert!(
        matches!(out.fns[0].body.stmts[2], TypedStmt::ReturnCleanup { .. }),
        "expected ReturnCleanup after hoisted stmts, got: {:?}", out.fns[0].body.stmts[2]
    );
}

#[test]
fn unwrap_or_receiver_call_hoists_strlit_arg() {
    // Regression: mk(true, "x").unwrap_or("fallback") — the "x" inside the
    // Call inside the UnwrapOr receiver must be materialized so every creation
    // claim gets a matching release at scope exit.
    let call = TypedExpr::new(
        TypedExprKind::Call {
            func: kai_tast::FunctionId(1),
            args: vec![str_lit("x")],
        },
        KaiType::Optional(Box::new(KaiType::String)),
    );
    let unwrap_or = TypedExpr::new(
        TypedExprKind::UnwrapOr {
            receiver: Box::new(call),
            default: Box::new(str_lit("fallback")),
        },
        KaiType::String,
    );
    let body = block(vec![TypedStmt::Expr(unwrap_or), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // stmts[0]: $tmp = "x"  (StrLit arg)
    let TypedStmt::Let(h0) = &out.fns[0].body.stmts[0]
    else { panic!("expected hoisted StrLit arg:\n{out:#?}") };
    assert!(matches!(h0.init.kind, TypedExprKind::StrLit { .. }));
    // stmts[1]: $tmp = Call(...)  (Call with Optional<String> return)
    let TypedStmt::Let(h1) = &out.fns[0].body.stmts[1]
    else { panic!("expected hoisted Call:\n{out:#?}") };
    assert!(matches!(h1.init.kind, TypedExprKind::Call { .. }));
    // stmts[2]: $tmp = UnwrapOr { receiver: LocalRef, default: StrLit("fallback") }
    let TypedStmt::Let(h2) = &out.fns[0].body.stmts[2]
    else { panic!("expected hoisted UnwrapOr:\n{out:#?}") };
    assert!(
        matches!(h2.init.kind, TypedExprKind::UnwrapOr { .. }),
        "third hoist must be the UnwrapOr, got: {:?}", h2.init.kind
    );
    // The default ("fallback") must NOT be separately hoisted — it's lazy.
    if let TypedExprKind::UnwrapOr { default, .. } = &h2.init.kind {
        assert!(
            matches!(default.kind, TypedExprKind::StrLit { .. }),
            "default must remain a StrLit (lazy), got: {:?}", default.kind
        );
    }
}

#[test]
fn catch_base_call_hoists_strlit_arg() {
    // Regression: mk(false, "q") catch |e| { e } — the "q" inside the
    // Call inside the Catch base must be materialized.
    let call = TypedExpr::new(
        TypedExprKind::Call {
            func: kai_tast::FunctionId(1),
            args: vec![str_lit("q")],
        },
        KaiType::Result {
            ok: Box::new(KaiType::String),
            err: Box::new(KaiType::String),
        },
    );
    let catch = TypedExpr::new(
        TypedExprKind::Catch {
            base: Box::new(call),
            err_binding: kai_tast::LocalId(2),
            err_ty: KaiType::String,
            stmts: vec![],
            tail: Box::new(local_ref(2, KaiType::String)),
            releases: vec![],
        },
        KaiType::String,
    );
    let body = block(vec![TypedStmt::Expr(catch), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // stmts[0]: $tmp = "q"  (StrLit arg)
    let TypedStmt::Let(h0) = &out.fns[0].body.stmts[0]
    else { panic!("expected hoisted StrLit arg:\n{out:#?}") };
    assert!(matches!(h0.init.kind, TypedExprKind::StrLit { .. }));
    // stmts[1]: $tmp = Call(...)  (Call with Result return)
    let TypedStmt::Let(h1) = &out.fns[0].body.stmts[1]
    else { panic!("expected hoisted Call:\n{out:#?}") };
    assert!(matches!(h1.init.kind, TypedExprKind::Call { .. }));
    // stmts[2]: Expr(Catch { base: LocalRef, ... })
    // Catch is NOT in is_owned_temp, so it is NOT materialized —
    // only the base (Call) inside it is.
    let TypedStmt::Expr(e) = &out.fns[0].body.stmts[2] else { panic!("expected Expr") };
    if let TypedExprKind::Catch { base, .. } = &e.kind {
        assert!(
            matches!(base.kind, TypedExprKind::LocalRef(_)),
            "Catch base must reference the Call's hidden local, got: {:?}", base.kind
        );
    } else {
        panic!("expected Catch expression, got: {:?}", e.kind);
    }
}

#[test]
fn nested_oklit_call_hoists_strlit_arg() {
    // Regression: Ok(Call(func, ["x"])) — nested owned temps must all be
    // materialized so no creation claim is orphaned.
    let call = TypedExpr::new(
        TypedExprKind::Call {
            func: kai_tast::FunctionId(1),
            args: vec![str_lit("x")],
        },
        KaiType::String,
    );
    let ok = TypedExpr::new(
        TypedExprKind::OkLit(Box::new(call)),
        KaiType::Result {
            ok: Box::new(KaiType::String),
            err: Box::new(KaiType::String),
        },
    );
    let body = block(vec![TypedStmt::Expr(ok), ret(None)]);
    let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
    let out = run(program);
    // stmts[0]: $tmp = "x"  (StrLit arg)
    let TypedStmt::Let(h0) = &out.fns[0].body.stmts[0]
    else { panic!("expected hoisted StrLit arg:\n{out:#?}") };
    assert!(matches!(h0.init.kind, TypedExprKind::StrLit { .. }));
    // stmts[1]: $tmp = Call(...)  (Call with String return)
    let TypedStmt::Let(h1) = &out.fns[0].body.stmts[1]
    else { panic!("expected hoisted Call:\n{out:#?}") };
    assert!(matches!(h1.init.kind, TypedExprKind::Call { .. }));
    // The OkLit wraps the Call's hidden local — OkLit IS an owned temp
    // so it IS materialized as a third hidden local.
    let TypedStmt::Let(h2) = &out.fns[0].body.stmts[2]
    else { panic!("expected hoisted OkLit:\n{out:#?}") };
    assert!(
        matches!(h2.init.kind, TypedExprKind::OkLit(_)),
        "third hoist must be the OkLit, got: {:?}", h2.init.kind
    );
}