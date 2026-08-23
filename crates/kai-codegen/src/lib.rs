//! LLVM codegen via inkwell. Consumes TAST only — the crate has no
//! dependency on `kai-ast` by construction (§8, constraint 2).

pub(crate) mod context;
pub(crate) mod emit;
pub(crate) mod frame;
pub(crate) mod module;
pub(crate) mod types;

use context::Ctx;
use kai_tast::TypedProgram;
use std::sync::OnceLock;

/// Compiles a typed program to textual LLVM IR, verifying the module first.
pub fn compile_ir(module_name: &str, program: &TypedProgram) -> Result<String, String> {
    let context = inkwell::context::Context::create();
    let mut ctx = Ctx::new(&context, module_name);

    emit::program(&mut ctx, program);
    module::verify(&ctx)?;
    Ok(module::print(&ctx))
}

/// JIT-compiles and runs `main`, returning its `int32` result.
pub fn run_jit(program: &TypedProgram) -> Result<i32, String> {
    initialize_native()?;

    let context = inkwell::context::Context::create();
    let mut ctx = Ctx::new(&context, "kai_jit");
    emit::program(&mut ctx, program);
    module::verify(&ctx)?;

    // The engine takes ownership of the module.
    let module = ctx.module;
    let engine = module
        .create_jit_execution_engine(inkwell::OptimizationLevel::None)
        .map_err(|e| e.to_string())?;

    unsafe {
        let main = engine
            .get_function::<unsafe extern "C" fn() -> i32>("main")
            .map_err(|e| e.to_string())?;
        Ok(main.call())
    }
}

fn initialize_native() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        inkwell::targets::Target::initialize_native(
            &inkwell::targets::InitializationConfig::default(),
        )
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_tast::{
        FunctionId, KaiType, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl, TypedProgram,
        TypedStmt,
    };

    fn minimal_program() -> TypedProgram {
        let ret_expr = TypedExpr::new(TypedExprKind::IntLit(0), KaiType::Int32);
        TypedProgram {
            structs: Vec::new(),
            fns: vec![TypedFnDecl {
                id: FunctionId(0),
                name: "main".into(),
                module: String::new(),
                params: Vec::new(),
                ret: KaiType::Int32,
                body: TypedBlock {
                    stmts: vec![TypedStmt::Return(Some(ret_expr))],
                },
            }],
        }
    }

    #[test]
    fn compiles_minimal_to_verified_ir() {
        let ir = compile_ir("test", &minimal_program()).unwrap();
        assert!(ir.contains("define i32 @main()"), "ir:\n{ir}");
        assert!(ir.contains("ret i32 0"), "ir:\n{ir}");
    }

    #[test]
    fn jits_minimal_and_returns_zero() {
        assert_eq!(run_jit(&minimal_program()).unwrap(), 0);
    }

    #[test]
    fn negative_literal_keeps_bit_pattern() {
        let expr = TypedExpr::new(TypedExprKind::IntLit(-1), KaiType::Int32);
        let program = TypedProgram {
            structs: Vec::new(),
            fns: vec![TypedFnDecl {
                id: FunctionId(0),
                name: "main".into(),
                module: String::new(),
                params: Vec::new(),
                ret: KaiType::Int32,
                body: TypedBlock {
                    stmts: vec![TypedStmt::Return(Some(expr))],
                },
            }],
        };
        assert_eq!(run_jit(&program).unwrap(), -1);
    }

    /// `id` must be unique per function: it indexes codegen's registry.
    pub(crate) fn fn_decl(id: u32, name: &str, ret: KaiType, stmts: Vec<TypedStmt>) -> TypedFnDecl {
        TypedFnDecl {
            id: FunctionId(id),
            name: name.into(),
            module: String::new(),
            params: Vec::new(),
            ret,
            body: TypedBlock { stmts },
        }
    }

    /// Codegen invariant check: every stack allocation must be emitted inside
    /// the entry block, even when the binding itself lives in a nested block.
    #[test]
    fn allocas_live_in_entry_block_only() {
        use kai_tast::{LocalId, TypedIf, TypedLet};

        let let_in_branch = TypedStmt::Let(TypedLet {
            local: LocalId(0),
            name: "x".into(),
            init: TypedExpr::new(TypedExprKind::IntLit(1), KaiType::Int32),
        });
        let ret = TypedStmt::Return(Some(TypedExpr::new(
            TypedExprKind::IntLit(0),
            KaiType::Int32,
        )));
        let program = TypedProgram {
            structs: Vec::new(),
            fns: vec![fn_decl(
                0,
                "main",
                KaiType::Int32,
                vec![
                    TypedStmt::If(TypedIf {
                        cond: TypedExpr::new(TypedExprKind::BoolLit(true), KaiType::Bool),
                        then_block: TypedBlock {
                            stmts: vec![let_in_branch],
                        },
                        else_block: None,
                    }),
                    ret,
                ],
            )],
        };

        let ir = compile_ir("test", &program).unwrap();

        let mut current_is_entry = false;
        let mut saw_entry_block = false;
        let mut total_allocas = 0usize;
        let mut outside_allocas = 0usize;
        for line in ir.lines() {
            if line.starts_with("define") {
                current_is_entry = false;
                continue;
            }
            if line.is_empty() || !line.starts_with(' ') {
                // Label lines start at column zero.
                if !line.is_empty() {
                    current_is_entry = line.trim_start().starts_with("entry:");
                    saw_entry_block |= current_is_entry;
                }
                continue;
            }
            if line.contains("alloca") {
                total_allocas += 1;
                if !current_is_entry {
                    outside_allocas += 1;
                }
            }
        }

        assert!(saw_entry_block, "no entry block found:\n{ir}");
        assert!(total_allocas > 0, "no allocas found:\n{ir}");
        assert_eq!(outside_allocas, 0, "alloca outside entry block:\n{ir}");
    }

    /// Designed behavior (§10.2 notes): a unit function's empty body falls
    /// through to `ret void`.
    #[test]
    fn unit_fn_empty_body_emits_ret_void() {
        let program = TypedProgram {
            structs: Vec::new(),
            fns: vec![
                fn_decl(
                    0,
                    "main",
                    KaiType::Int32,
                    vec![TypedStmt::Return(Some(TypedExpr::new(
                        TypedExprKind::IntLit(0),
                        KaiType::Int32,
                    )))],
                ),
                fn_decl(1, "side_effect_free", KaiType::Unit, vec![]),
            ],
        };
        let ir = compile_ir("test", &program).unwrap();
        assert!(ir.contains("define void @side_effect_free()"), "ir:\n{ir}");
        assert!(ir.contains("ret void"), "ir:\n{ir}");
        assert_eq!(run_jit(&program).unwrap(), 0);
    }

    #[test]
    fn unit_fn_bare_return_emits_ret_void() {
        let program = TypedProgram {
            structs: Vec::new(),
            fns: vec![
                fn_decl(
                    0,
                    "main",
                    KaiType::Int32,
                    vec![TypedStmt::Return(Some(TypedExpr::new(
                        TypedExprKind::IntLit(0),
                        KaiType::Int32,
                    )))],
                ),
                fn_decl(1, "early_out", KaiType::Unit, vec![TypedStmt::Return(None)]),
            ],
        };
        let ir = compile_ir("test", &program).unwrap();
        assert!(ir.contains("define void @early_out()"), "ir:\n{ir}");
        assert_eq!(run_jit(&program).unwrap(), 0);
    }
}

#[cfg(test)]
mod v0003_tests {
    use super::*;
    use kai_tast::{
        FieldStep, FunctionId, KaiType, LocalId, StructId, TypedBlock, TypedExpr, TypedExprKind,
        TypedFnDecl, TypedParam, TypedProgram, TypedStmt, TypedStruct, TypedStructField,
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
            body: TypedBlock {
                stmts: vec![
                    TypedStmt::Assign(kai_tast::TypedAssign {
                        root: LocalId(0),
                        path: vec![FieldStep {
                            struct_id: StructId(0),
                            field: 0,
                        }],
                        op: Some(kai_tast::BinaryOp::Add),
                        value: TypedExpr::new(TypedExprKind::IntLit(5), KaiType::Int32),
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
}

#[cfg(test)]
mod v0004_tests {
    use super::*;
    use kai_tast::{FunctionId, KaiType, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl,
        TypedProgram, TypedStmt, TypedStruct};

    fn bare(id: u32, name: &str, module: &str, ret: KaiType, value: i64) -> TypedFnDecl {
        let mut decl = tests::fn_decl(
            id,
            name,
            ret,
            vec![TypedStmt::Return(Some(TypedExpr::new(
                TypedExprKind::IntLit(value),
                ret,
            )))],
        );
        decl.module = module.to_string();
        decl
    }

    #[test]
    fn same_named_fns_in_different_modules_never_collide() {
        let program = TypedProgram {
            structs: Vec::new(),
            fns: vec![
                bare(0, "main", "", KaiType::Int32, 7),
                bare(1, "dup", "lib.a", KaiType::Int32, 1),
                bare(2, "dup", "lib.b", KaiType::Int32, 2),
            ],
        };
        let ir = compile_ir("test", &program).expect("qualified symbols are distinct");
        assert!(ir.contains("@lib.a.dup"), "ir:\n{ir}");
        assert!(ir.contains("@lib.b.dup"), "ir:\n{ir}");
        // Entry names stay bare so JIT can find @main.
        assert!(ir.contains("define i32 @main()"), "ir:\n{ir}");
        assert_eq!(run_jit(&program).unwrap(), 7);
    }

    #[test]
    fn qualified_struct_type_names_follow_the_same_rule() {
        let point = TypedStruct {
            name: "Point".into(),
            module: "support.math".into(),
            fields: vec![
                kai_tast::TypedStructField {
                    name: "x".into(),
                    ty: KaiType::Int32,
                },
                kai_tast::TypedStructField {
                    name: "y".into(),
                    ty: KaiType::Int32,
                },
            ],
        };
        // A signature referencing the struct forces it into the printed IR.
        let takes_point = TypedFnDecl {
            id: FunctionId(1),
            name: "peek".into(),
            module: "support.math".into(),
            params: vec![kai_tast::TypedParam {
                local: kai_tast::LocalId(0),
                name: "p".into(),
                ty: KaiType::Struct(kai_tast::StructId(0)),
            }],
            ret: KaiType::Unit,
            body: TypedBlock {
                stmts: vec![TypedStmt::Return(None)],
            },
        };
        let program = TypedProgram {
            structs: vec![point],
            fns: vec![bare(0, "main", "", KaiType::Int32, 0), takes_point],
        };
        let ir = compile_ir("test", &program).expect("compiles");
        assert!(
            ir.contains("%support.math.Point = type"),
            "ir:\n{ir}"
        );
        assert!(ir.contains("@support.math.peek"), "ir:\n{ir}");
    }
}
