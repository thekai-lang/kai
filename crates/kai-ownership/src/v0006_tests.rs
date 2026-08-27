use super::*;
    use kai_tast::TypedClosure;

    fn str_lit(s: &str) -> TypedExpr {
        TypedExpr::new(TypedExprKind::StrLit { value: s.into() }, KaiType::String)
    }
    fn int_lit(v: i64) -> TypedExpr {
        TypedExpr::new(TypedExprKind::IntLit(v), KaiType::Int32)
    }
    fn local_ref(id: u32, ty: KaiType) -> TypedExpr {
        TypedExpr::new(TypedExprKind::LocalRef(LocalId(id)), ty)
    }
    fn let_(id: u32, name: &str, init: TypedExpr) -> TypedStmt {
        TypedStmt::Let(kai_tast::TypedLet { local: LocalId(id), name: name.into(), init })
    }
    fn ret(e: Option<TypedExpr>) -> TypedStmt {
        TypedStmt::Return(e)
    }
    fn block(stmts: Vec<TypedStmt>) -> TypedBlock {
        TypedBlock { stmts }
    }
    fn heap_table() -> HeapBearing {
        HeapBearing::new(&[])
    }
    fn run(body: Vec<TypedStmt>) -> Vec<TypedStmt> {
        let heap = heap_table();
        let mut fresh = FreshIds::default();
        let mut scopes = Scopes::default();
        walk_block(&heap, block(body), &mut scopes, &mut fresh, false).stmts
    }

    #[test]
    fn some_payload_is_an_owning_slot() {
        // `let o: string? = Some(name);` — the payload borrows `name`, so
        // construction retains it (§9.5 row 3 generalized to tagged unions).
        let mut some = TypedExpr {
            kind: TypedExprKind::SomeLit(Box::new(local_ref(0, KaiType::String))),
            ty: KaiType::Optional(Box::new(KaiType::String)),
            span: kai_diagnostics::Span::new(0, 0),
        };
        let body = vec![let_(1, "o", some.clone()), ret(None)];
        let out = run(body);
        match &out[0] {
            TypedStmt::Let(l) => match &l.init.kind {
                TypedExprKind::SomeLit(inner) => assert!(
                    matches!(inner.kind, TypedExprKind::Retain(_)),
                    "payload must be retained at construction"
                ),
                other => panic!("expected SomeLit, got {other:?}"),
            },
            other => panic!("expected let, got {other:?}"),
        }
        // The binding releases on the return path like any heap local.
        let released_through_return = matches!(
            &out[1],
            TypedStmt::ReturnCleanup { releases, .. }
                if releases.iter().any(|(l, _)| *l == LocalId(1))
        );
        assert!(released_through_return, "got {:?}", out[1]);
        let _ = &mut some;
    }

    #[test]
    fn closure_literal_counts_as_owned_temporary() {
        // Discarding a closure literal binds it to a hidden `$tmp` so the
        // environment is released at statement end (unconditionally
        // heap-bearing, §9.10).
        let clo = TypedExpr {
            kind: TypedExprKind::ClosureLit(Box::new(kai_tast::TypedClosure {
                param_ids: vec![],
                body: block(vec![ret(Some(int_lit(0)))]),
                captures: vec![],
            })),
            ty: KaiType::Closure { params: vec![], ret: Box::new(KaiType::Int32) },
            span: kai_diagnostics::Span::new(0, 0),
        };
        let out = run(vec![
            TypedStmt::Expr(clo),
            ret(None),
        ]);
        assert!(
            matches!(&out[0], TypedStmt::Let(l) if l.name == "$tmp"),
            "closure literal must materialize when discarded: {:?}",
            out[0]
        );
    }

    #[test]
    fn coalesce_fallback_is_materialized_for_leak_prevention() {
        // v0.0.8.3: Coalesce IS an owned temp (is_owned_temp=true) — it gets
        // materialized into a hidden local so scope machinery releases it.
        // This prevents the per-iteration leak found by ASan audit (BUG-5).
        // Lazy evaluation is preserved: the Coalesce node survives as the
        // Let init, and lazy_select still branches internally.
        let mut co = TypedExpr {
            kind: TypedExprKind::Coalesce {
                lhs: Box::new(local_ref(0, KaiType::String)),
                rhs: Box::new(str_lit("d")),
            },
            ty: KaiType::String,
            span: kai_diagnostics::Span::new(0, 0),
        };
        let heap = heap_table();
        let mut fresh = FreshIds::default();
        let mut scopes = Scopes::default();
        scopes.push(); // production always has a scope open (walk_block pushes)
        let mut pre = Vec::new();
        hoist_borrow_temps(&heap, &mut co, &mut fresh, &mut scopes, &mut pre, false);
        // The Coalesce IS materialized into a hidden $tmp local.
        assert_eq!(pre.len(), 1, "coalesce must be materialized for leak tracking");
        if let TypedStmt::Let(binding) = &pre[0] {
            assert_eq!(binding.name, "$tmp");
            assert!(
                matches!(binding.init.kind, TypedExprKind::Coalesce { .. }),
                "init must be the original Coalesce node"
            );
        } else {
            panic!("expected Let statement in pre");
        }
        // The outer expression is now a LocalRef to the hidden local.
        assert!(
            matches!(co.kind, TypedExprKind::LocalRef(_)),
            "outer must reference the hidden local"
        );
    }

    #[test]
    fn catch_block_locals_release_after_the_tail() {
        // A string declared inside the catch block is released only after
        // the tail consumed it — encoded in Catch.releases.
        let mut catch_expr = TypedExpr {
            kind: TypedExprKind::Catch {
                base: Box::new(local_ref(0, KaiType::Result {
                    ok: Box::new(KaiType::Int32),
                    err: Box::new(KaiType::String),
                })),
                err_binding: LocalId(90),
                err_ty: KaiType::String,
                stmts: vec![let_(91, "$s", str_lit("log"))],
                tail: Box::new(int_lit(7)),
                releases: vec![],
            },
            ty: KaiType::Int32,
            span: kai_diagnostics::Span::new(0, 0),
        };
        let heap = heap_table();
        let mut fresh = FreshIds::default();
        let mut scopes = Scopes::default();
        scopes.push(); // function root
        walk_expr(&heap, &mut catch_expr, &mut scopes, &mut fresh, false);
        scopes.pop();
        match catch_expr.kind {
            TypedExprKind::Catch { releases, .. } => {
                assert_eq!(releases.len(), 1, "{releases:?}");
                assert_eq!(releases[0].0, LocalId(91));
            }
            other => panic!("expected catch, got {other:?}"),
        }
    }

    #[test]
    fn capture_retains_are_codegen_keyed_not_pass_nodes() {
        // Contract test: a closure literal with heap captures passes through
        // WITHOUT Retain wrappers — codegen retains per capture type at env
        // construction (compile-time keyed, §9.9a's one-mechanism rule).
        let clo = TypedClosure {
            param_ids: vec![],
            body: block(vec![ret(Some(local_ref(3, KaiType::String)))]),
            captures: vec![kai_tast::TypedCapture {
                local: LocalId(3),
                ty: KaiType::String,
            }],
        };
        let mut e = TypedExpr {
            kind: TypedExprKind::ClosureLit(Box::new(clo)),
            ty: KaiType::Closure { params: vec![], ret: Box::new(KaiType::String) },
            span: kai_diagnostics::Span::new(0, 0),
        };
        let heap = heap_table();
        let mut fresh = FreshIds::default();
        let mut scopes = Scopes::default();
        scopes.push();
        walk_expr(&heap, &mut e, &mut scopes, &mut fresh, false);
        assert!(matches!(e.kind, TypedExprKind::ClosureLit(_)));
    }
