//! LLVM codegen via inkwell. Consumes TAST only — the crate has no
//! dependency on `kai-ast` by construction (§8, constraint 2).

pub(crate) mod context;
pub(crate) mod emit;
pub(crate) mod frame;
pub(crate) mod module;
pub(crate) mod runtime;
pub(crate) mod types;

use context::{Ctx, SourceInfo};
use kai_tast::TypedProgram;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One module's source, keyed for panic locations (§10.1): dotted module
/// name (`""` = entry), the display path reported in messages, and the
/// text spans resolve against. Programs compiled without sources simply
/// pass an empty slice — every location degrades to `<unknown>:0:0`.
pub struct SourceUnit {
    pub module: String,
    pub file: String,
    pub text: String,
}

fn source_map(sources: &[SourceUnit]) -> HashMap<String, SourceInfo> {
    sources
        .iter()
        .map(|unit| {
            (
                unit.module.clone(),
                SourceInfo::new(&unit.file, &unit.text),
            )
        })
        .collect()
}

/// Compiles a typed program to textual LLVM IR, verifying the module first.
pub fn compile_ir(module_name: &str, program: &TypedProgram) -> Result<String, String> {
    compile_ir_with_sources(module_name, program, &[])
}

/// [`compile_ir`] with per-module sources attached, so runtime checks can
/// bake real `file:line:col` panic sites (§10.1).
pub fn compile_ir_with_sources(
    module_name: &str,
    program: &TypedProgram,
    sources: &[SourceUnit],
) -> Result<String, String> {
    let context = inkwell::context::Context::create();
    let mut ctx = Ctx::new(&context, module_name, source_map(sources));

    emit::program(&mut ctx, program);
    module::verify(&ctx)?;
    Ok(module::print(&ctx))
}

/// JIT-compiles and runs `main`, returning its `int32` result.
pub fn run_jit(program: &TypedProgram) -> Result<i32, String> {
    run_jit_with_sources(program, &[])
}

/// [`run_jit`] with per-module sources attached (see
/// [`compile_ir_with_sources`]).
pub fn run_jit_with_sources(
    program: &TypedProgram,
    sources: &[SourceUnit],
) -> Result<i32, String> {
    initialize_native()?;

    let context = inkwell::context::Context::create();
    let mut ctx = Ctx::new(&context, "kai_jit", source_map(sources));
    emit::program(&mut ctx, program);
    module::verify(&ctx)?;

    // The engine takes ownership of the module.
    let module = ctx.module;
    let engine = module
        .create_jit_execution_engine(inkwell::OptimizationLevel::None)
        .map_err(|e| e.to_string())?;

    // Bind the runtime intrinsics explicitly: the linker is free to strip
    // `#[no_mangle]` extern fns nothing in Rust references, so dlsym alone
    // cannot be trusted. Taking their addresses here also pins them into
    // the final binary.
    for (name, addr) in runtime::INTRINSICS {
        if let Some(f) = module.get_function(name) {
            engine.add_global_mapping(&f, addr as usize);
        }
    }

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
mod tests;
#[cfg(test)]
mod v0003_tests;
#[cfg(test)]
mod v0004_tests;
#[cfg(test)]
mod v0005_panic_tests;
