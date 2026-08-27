use super::*;
    use kai_tast::{EffectSet, BinaryOp, StructId, TypedStruct, TypedStructField};

    // ---------- hand-built TAST helpers ----------

    pub(crate) fn str_lit(s: &str) -> TypedExpr {
        TypedExpr::new(TypedExprKind::StrLit { value: s.into() }, KaiType::String)
    }

    pub(crate) fn int_lit(v: i64) -> TypedExpr {
        TypedExpr::new(TypedExprKind::IntLit(v), KaiType::Int32)
    }

    pub(crate) fn local_ref(id: u32, ty: KaiType) -> TypedExpr {
        TypedExpr::new(TypedExprKind::LocalRef(LocalId(id)), ty)
    }

    pub(crate) fn let_(id: u32, name: &str, init: TypedExpr) -> TypedStmt {
        TypedStmt::Let(kai_tast::TypedLet {
            local: LocalId(id),
            name: name.into(),
            init,
        })
    }

    pub(crate) fn assign(root: u32, path: Vec<kai_tast::TypedPlaceStep>, value: TypedExpr) -> TypedAssign {
        TypedAssign {
            root: LocalId(root),
            path,
            op: None,
            value,
            release_old: false,
            span: kai_diagnostics::Span::new(0, 0),
        }
    }

    pub(crate) fn ret(e: Option<TypedExpr>) -> TypedStmt {
        TypedStmt::Return(e)
    }

    pub(crate) fn block(stmts: Vec<TypedStmt>) -> TypedBlock {
        TypedBlock { stmts }
    }

    pub(crate) fn fn_decl(body: TypedBlock, params: Vec<kai_tast::TypedParam>, ret_ty: KaiType) -> TypedFnDecl {
        TypedFnDecl {
            id: kai_tast::FunctionId(0),
            name: "main".into(),
            module: String::new(),
            params,
            ret: ret_ty,
            declared_effects: None,
            inferred_effects: EffectSet::default(),
            is_reversible: false,
            body,
        }
    }

    pub(crate) fn param(id: u32, name: &str, ty: KaiType) -> kai_tast::TypedParam {
        kai_tast::TypedParam {
            local: LocalId(id),
            name: name.into(),
            ty,
        }
    }

    pub(crate) fn run(mut program: TypedProgram) -> TypedProgram {
        resolve(&mut program);
        program
    }

    pub(crate) fn unwrap_retain(e: &TypedExpr) -> bool {
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
