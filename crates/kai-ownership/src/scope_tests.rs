//! Scope-exit release ordering (§9.4) and for..in ownership (§9.9).

use super::tests::{block, fn_decl, int_lit, let_, local_ref, ret, run, str_lit, unwrap_retain};
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