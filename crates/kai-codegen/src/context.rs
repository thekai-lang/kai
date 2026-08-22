use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;

/// Bundles the per-compilation LLVM objects. The `Context` outlives this
/// struct at the call site; everything here borrows it.
pub(crate) struct Ctx<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
}

impl<'ctx> Ctx<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
        }
    }
}
