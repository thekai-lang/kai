use super::*;
    use kai_tast::{BinaryOp, StructId, TypedStruct, TypedStructField};

    // ---------- hand-built TAST helpers ----------

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
        TypedStmt::Let(kai_tast::TypedLet {
            local: LocalId(id),
            name: name.into(),
            init,
        })
    }

    fn assign(root: u32, path: Vec<kai_tast::TypedPlaceStep>, value: TypedExpr) -> TypedAssign {
        TypedAssign {
            root: LocalId(root),
            path,
            op: None,
            value,
            release_old: false,
            span: kai_diagnostics::Span::new(0, 0),
        }
    }

    fn ret(e: Option<TypedExpr>) -> TypedStmt {
        TypedStmt::Return(e)
    }

    fn block(stmts: Vec<TypedStmt>) -> TypedBlock {
        TypedBlock { stmts }
    }

    fn fn_decl(body: TypedBlock, params: Vec<kai_tast::TypedParam>, ret_ty: KaiType) -> TypedFnDecl {
        TypedFnDecl {
            id: kai_tast::FunctionId(0),
            name: "main".into(),
            module: String::new(),
            params,
            ret: ret_ty,
            body,
        }
    }

    fn param(id: u32, name: &str, ty: KaiType) -> kai_tast::TypedParam {
        kai_tast::TypedParam {
            local: LocalId(id),
            name: name.into(),
            ty,
        }
    }

    fn run(mut program: TypedProgram) -> TypedProgram {
        resolve(&mut program);
        program
    }

    fn unwrap_retain(e: &TypedExpr) -> bool {
        matches!(e.kind, TypedExprKind::Retain(_))
    }

    // ---------- heap-bearing table ----------

    #[test]
    fn heap_bearing_classification() {
        let structs = vec![
            TypedStruct {
                name: "Plain".into(),
                module: String::new(),
                fields: vec![
                    TypedStructField { name: "x".into(), ty: KaiType::Int32 },
                    TypedStructField { name: "y".into(), ty: KaiType::Bool },
                ],
            },
            TypedStruct {
                name: "Bearing".into(),
                module: String::new(),
                fields: vec![TypedStructField { name: "s".into(), ty: KaiType::String }],
            },
            // Forward reference: declared BEFORE the struct it embeds.
            TypedStruct {
                name: "Outer".into(),
                module: String::new(),
                fields: vec![TypedStructField {
                    name: "inner".into(),
                    ty: KaiType::Struct(StructId(4)),
                }],
            },
            TypedStruct {
                name: "Empty".into(),
                module: String::new(),
                fields: vec![],
            },
            TypedStruct {
                name: "Inner".into(),
                module: String::new(),
                fields: vec![TypedStructField { name: "a".into(), ty: KaiType::Array(Box::new(KaiType::Int32)) }],
            },
        ];
        let heap = HeapBearing::new(&structs);
        assert!(!heap.is(&KaiType::Struct(StructId(0)))); // Plain
        assert!(heap.is(&KaiType::Struct(StructId(1))));  // Bearing
        assert!(heap.is(&KaiType::Struct(StructId(2))));  // Outer (forward ref)
        assert!(!heap.is(&KaiType::Struct(StructId(3)))); // Empty
        assert!(!heap.is(&KaiType::Int32));
        assert!(heap.is(&KaiType::Array(Box::new(KaiType::Int32))));
        assert!(heap.is(&KaiType::String));
    }

    // ---------- retain-on-transfer (§9.5 / E8) ----------

    #[test]
    fn returning_param_retains() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(local_ref(0, KaiType::String)))]),
                vec![param(0, "s", KaiType::String)],
                KaiType::String,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, releases } = &out.fns[0].body.stmts[0]
        else {
            panic!("expected return cleanup");
        };
        let inner = value.as_ref().expect("return keeps its value");
        assert!(unwrap_retain(inner));
        let TypedExprKind::Retain(inner) = &inner.kind else { panic!("expected retain") };
        assert!(matches!(inner.kind, TypedExprKind::LocalRef(_)));
        assert_eq!(inner.ty, KaiType::String);
        // Params borrow — they are never in any release list.
        assert!(releases.is_empty());
    }

    #[test]
    fn returning_literal_moves_free() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(str_lit("hi")))]),
                vec![],
                KaiType::String,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().expect("literal survives the return");
        assert!(!unwrap_retain(e));
    }

    #[test]
    fn scalar_returns_never_retain() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(local_ref(0, KaiType::Int32)))]),
                vec![param(0, "n", KaiType::Int32)],
                KaiType::Int32,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().unwrap();
        assert!(!unwrap_retain(e));
    }

    #[test]
    fn let_of_binding_co_owns_via_retain() {
        // let x = "a"; let y = x;
        let body = block(vec![
            let_(0, "x", str_lit("a")),
            let_(1, "y", local_ref(0, KaiType::String)),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Let(y) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(unwrap_retain(&y.init));
        // x stays unwrapped (owned temp moves free).
        let TypedStmt::Let(x) = &out.fns[0].body.stmts[0] else { panic!() };
        assert!(!unwrap_retain(&x.init));
    }

    #[test]
    fn assignment_retains_borrowed_and_marks_release_old() {
        // var v = "a"; v = w;   (w is another binding)
        let body = block(vec![
            let_(0, "v", str_lit("a")),
            let_(1, "w", str_lit("b")),
            TypedStmt::Assign(assign(0, vec![], local_ref(1, KaiType::String))),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[2] else { panic!() };
        assert!(a.release_old, "owning slot replacement releases old (E4)");
        assert!(unwrap_retain(&a.value), "borrowed RHS retains before move");
    }

    #[test]
    fn owned_temp_assignment_moves_free_but_still_releases_old() {
        let body = block(vec![
            let_(0, "v", str_lit("a")),
            TypedStmt::Assign(assign(0, vec![], str_lit("fresh"))),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(a.release_old);
        assert!(!unwrap_retain(&a.value));
    }

    #[test]
    fn compound_assign_never_sets_release_old() {
        let a = TypedAssign {
            root: LocalId(0),
            path: vec![],
            op: Some(BinaryOp::Add),
            value: int_lit(1),
            release_old: false,
            span: kai_diagnostics::Span::new(0, 0),
        };
        let body = block(vec![
            let_(0, "n", int_lit(0)),
            TypedStmt::Assign(a),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(!a.release_old);
    }

    #[test]
    fn literal_fields_and_elements_are_owning_slots() {
        // wrap(p): User { name: p }  and  [p, p]
        let user_struct = TypedStruct {
            name: "User".into(),
            module: String::new(),
            fields: vec![TypedStructField { name: "name".into(), ty: KaiType::String }],
        };
        let struct_lit = |values| TypedExpr::new(
            TypedExprKind::StructLit { struct_id: StructId(0), values },
            KaiType::Struct(StructId(0)),
        );
        let arr_lit = |elements| TypedExpr::new(
            TypedExprKind::ArrayLit { elements },
            KaiType::Array(Box::new(KaiType::String)),
        );

        let body = block(vec![ret(Some(struct_lit(vec![local_ref(0, KaiType::String)])))]);
        let program = TypedProgram {
            structs: vec![user_struct.clone()],
            fns: vec![fn_decl(body, vec![param(0, "p", KaiType::String)], KaiType::Struct(StructId(0)))],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else {
            panic!("literal itself moves free")
        };
        let e = value.as_ref().unwrap();
        assert!(!unwrap_retain(e));
        let TypedExprKind::StructLit { values, .. } = &e.kind else { panic!() };
        assert!(unwrap_retain(&values[0]), "field slot retains borrowed source");

        let body = block(vec![ret(Some(arr_lit(vec![local_ref(0, KaiType::String)])))]);
        let program = TypedProgram {
            structs: vec![user_struct],
            fns: vec![fn_decl(
                body,
                vec![param(0, "p", KaiType::String)],
                KaiType::Array(Box::new(KaiType::String)),
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().unwrap();
        let TypedExprKind::ArrayLit { elements } = &e.kind else { panic!() };
        assert!(unwrap_retain(&elements[0]), "array elements are owning slots");
    }

    #[test]
    fn call_arguments_are_borrowed_never_retained() {
        // callee(p); — argument position borrows (§9.6)
        let callee_call = TypedExpr::new(
            TypedExprKind::Call {
                func: kai_tast::FunctionId(1),
                args: vec![local_ref(0, KaiType::String)],
            },
            KaiType::Unit,
        );
        let body = block(vec![TypedStmt::Expr(callee_call), ret(None)]);
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(body, vec![param(0, "p", KaiType::String)], KaiType::Unit)],
        };
        let out = run(program);
        let TypedStmt::Expr(e) = &out.fns[0].body.stmts[0] else { panic!() };
        let TypedExprKind::Call { args, .. } = &e.kind else { panic!() };
        assert!(!unwrap_retain(&args[0]));
    }

    // ---------- scope-exit releases (§9.4) ----------

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
