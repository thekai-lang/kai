//! LLVM codegen via inkwell. Consumes TAST only — the crate has no
//! dependency on `kai-ast` by construction (§8, constraint 2).

pub(crate) mod context;
pub(crate) mod emit;
pub(crate) mod module;
pub(crate) mod types;

use context::Ctx;
use kai_tast::TypedProgram;
use std::sync::OnceLock;

/// Compiles a typed program to textual LLVM IR, verifying the module first.
pub fn compile_ir(module_name: &str, program: &TypedProgram) -> Result<String, String> {
    let context = inkwell::context::Context::create();
    let ctx = Ctx::new(&context, module_name);

    emit::program(&ctx, program);
    module::verify(&ctx)?;
    Ok(module::print(&ctx))
}

/// JIT-compiles and runs `main`, returning its `int32` result.
pub fn run_jit(program: &TypedProgram) -> Result<i32, String> {
    initialize_native()?;

    let context = inkwell::context::Context::create();
    let ctx = Ctx::new(&context, "kai_jit");
    emit::program(&ctx, program);
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
            fns: vec![TypedFnDecl {
                id: FunctionId(0),
                name: "main".into(),
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
            fns: vec![TypedFnDecl {
                id: FunctionId(0),
                name: "main".into(),
                ret: KaiType::Int32,
                body: TypedBlock {
                    stmts: vec![TypedStmt::Return(Some(expr))],
                },
            }],
        };
        assert_eq!(run_jit(&program).unwrap(), -1);
    }
}
