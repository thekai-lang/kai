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
