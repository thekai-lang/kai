use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

/// Bundles the per-compilation LLVM objects plus the id-keyed registries
/// filled during declaration. The `Context` outlives this struct at the call
/// site; everything here borrows it.
pub(crate) struct Ctx<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    /// LLVM struct types by `StructId` (declaration order).
    pub structs: Vec<StructType<'ctx>>,
    /// Declared functions by `FunctionId` (declaration order).
    pub functions: Vec<FunctionValue<'ctx>>,
}

impl<'ctx> Ctx<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            structs: Vec::new(),
            functions: Vec::new(),
        }
    }
}
