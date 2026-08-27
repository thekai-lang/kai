use super::*;
use kai_tast::{EffectSet, 
    FieldStep, FunctionId, KaiType, LocalId, StructId, TypedBlock, TypedExpr, TypedExprKind,
    TypedFnDecl, TypedParam, TypedPlaceStep, TypedProgram, TypedStmt, TypedStruct,
    TypedStructField,
};

// Reuse the parent test module's declaration helper.
use tests::fn_decl;

fn point_struct() -> TypedStruct {
    TypedStruct {
        name: "Point".into(),
        module: String::new(),
        fields: vec![
            TypedStructField {
                name: "x".into(),
                ty: KaiType::Int32,
            },
            TypedStructField {
                name: "y".into(),
                ty: KaiType::Int32,
            },
        ],
    }
}

#[test]
fn declares_named_llvm_struct_type() {
    let program = TypedProgram {
        structs: vec![point_struct()],
        fns: vec![fn_decl(
            0,
            "main",
            KaiType::Int32,
            vec![
                TypedStmt::Let(kai_tast::TypedLet {
                    local: LocalId(0),
                    name: "p".into(),
                    init: TypedExpr::new(
                        TypedExprKind::StructLit {
                            struct_id: StructId(0),
                            values: vec![
                                TypedExpr::new(TypedExprKind::IntLit(7), KaiType::Int32),
                                TypedExpr::new(TypedExprKind::IntLit(8), KaiType::Int32),
                            ],
                        },
                        KaiType::Struct(StructId(0)),
                    ),
                }),
                TypedStmt::Return(Some(TypedExpr::new(
                    TypedExprKind::FieldAccess {
                        base: Box::new(TypedExpr::new(
                            TypedExprKind::LocalRef(LocalId(0)),
                            KaiType::Struct(StructId(0)),
                        )),
                        struct_id: StructId(0),
                        field: 1,
                    },
                    KaiType::Int32,
                ))),
            ],
        )],
    };
    let ir = compile_ir("test", &program).unwrap();
    assert!(ir.contains("%Point = type { i32, i32 }"), "ir:\n{ir}");
    // Literal construction + field read round-trip through memory.
    assert_eq!(run_jit(&program).unwrap(), 8);
}

#[test]
fn jit_params_are_copies_and_field_places_write_locally() {
    // §9.3: struct params pass BY VALUE. `bump` mutates its own copy;
    // the caller's `p.x` must still read 1.
    let bump = TypedFnDecl {
        id: FunctionId(0),
        name: "bump".into(),
        module: String::new(),
        params: vec![TypedParam {
            local: LocalId(0),
            name: "p".into(),
            ty: KaiType::Struct(StructId(0)),
        }],
        ret: KaiType::Unit,
        declared_effects: None,
        inferred_effects: EffectSet::default(),
        is_reversible: false,
        body: TypedBlock {
            stmts: vec![
                TypedStmt::Assign(kai_tast::TypedAssign {
                    root: LocalId(0),
                    path: vec![TypedPlaceStep::Field(FieldStep {
                        struct_id: StructId(0),
                        field: 0,
                    })],
                    op: Some(kai_tast::BinaryOp::Add),
                    value: TypedExpr::new(TypedExprKind::IntLit(5), KaiType::Int32),
                    release_old: false,
                    span: kai_diagnostics::Span::new(0, 0),
                }),
                TypedStmt::Return(None),
            ],
        },
    };
    let main = TypedFnDecl {
        id: FunctionId(1),
        name: "main".into(),
        module: String::new(),
        params: Vec::new(),
        ret: KaiType::Int32,
        declared_effects: None,
        inferred_effects: EffectSet::default(),
        is_reversible: false,
        body: TypedBlock {
            stmts: vec![
                TypedStmt::Let(kai_tast::TypedLet {
                    local: LocalId(1),
                    name: "p".into(),
                    init: TypedExpr::new(
                        TypedExprKind::StructLit {
                            struct_id: StructId(0),
                            values: vec![
                                TypedExpr::new(TypedExprKind::IntLit(1), KaiType::Int32),
                                TypedExpr::new(TypedExprKind::IntLit(2), KaiType::Int32),
                            ],
                        },
                        KaiType::Struct(StructId(0)),
                    ),
                }),
                TypedStmt::Expr(TypedExpr::new(
                    TypedExprKind::Call {
                        func: FunctionId(0),
                        args: vec![TypedExpr::new(
                            TypedExprKind::LocalRef(LocalId(1)),
                            KaiType::Struct(StructId(0)),
                        )],
                    },
                    KaiType::Unit,
                )),
                TypedStmt::Return(Some(TypedExpr::new(
                    TypedExprKind::FieldAccess {
                        base: Box::new(TypedExpr::new(
                            TypedExprKind::LocalRef(LocalId(1)),
                            KaiType::Struct(StructId(0)),
                        )),
                        struct_id: StructId(0),
                        field: 0,
                    },
                    KaiType::Int32,
                ))),
            ],
        },
    };

    let program = TypedProgram {
        structs: vec![point_struct()],
        fns: vec![bump, main],
    };
    assert_eq!(run_jit(&program).unwrap(), 1);
}

#[test]
fn jit_call_argument_flows_into_result() {
    let add = TypedFnDecl {
        id: FunctionId(0),
        name: "add".into(),
        module: String::new(),
        params: vec![
            TypedParam {
                local: LocalId(0),
                name: "a".into(),
                ty: KaiType::Int32,
            },
            TypedParam {
                local: LocalId(1),
                name: "b".into(),
                ty: KaiType::Int32,
            },
        ],
        ret: KaiType::Int32,
        declared_effects: None,
        inferred_effects: EffectSet::default(),
        is_reversible: false,
        body: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(TypedExpr::new(
                TypedExprKind::Binary {
                    op: kai_tast::BinaryOp::Add,
                    lhs: Box::new(TypedExpr::new(
                        TypedExprKind::LocalRef(LocalId(0)),
                        KaiType::Int32,
                    )),
                    rhs: Box::new(TypedExpr::new(
                        TypedExprKind::LocalRef(LocalId(1)),
                        KaiType::Int32,
                    )),
                    rhs_hoists: Vec::new(),
                },
                KaiType::Int32,
            )))],
        },
    };
    let main = TypedFnDecl {
        id: FunctionId(1),
        name: "main".into(),
        module: String::new(),
        params: Vec::new(),
        ret: KaiType::Int32,
        declared_effects: None,
        inferred_effects: EffectSet::default(),
        is_reversible: false,
        body: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(TypedExpr::new(
                TypedExprKind::Call {
                    func: FunctionId(0),
                    args: vec![
                        TypedExpr::new(TypedExprKind::IntLit(20), KaiType::Int32),
                        TypedExpr::new(TypedExprKind::IntLit(22), KaiType::Int32),
                    ],
                },
                KaiType::Int32,
            )))],
        },
    };

    let program = TypedProgram {
        structs: Vec::new(),
        fns: vec![add, main],
    };
    assert_eq!(run_jit(&program).unwrap(), 42);
}
