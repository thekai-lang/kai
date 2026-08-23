use std::collections::HashMap;

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
    /// Kai field types per `StructId` (parallel to `structs`) — the
    /// ownership helpers recurse through these.
    pub struct_fields: Vec<Vec<kai_tast::KaiType>>,
    /// Declared functions by `FunctionId` (declaration order).
    pub functions: Vec<FunctionValue<'ctx>>,
    /// Lazily generated ownership helpers, keyed by type stem. RefCell
    /// because helper generation recurses while other code holds &Ctx.
    pub retain_helpers: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    pub release_helpers: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    pub elem_dtors: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
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
            struct_fields: Vec::new(),
            functions: Vec::new(),
            retain_helpers: Default::default(),
            release_helpers: Default::default(),
            elem_dtors: Default::default(),
        }
    }

    /// Registers a struct's Kai field types alongside its LLVM shape.
    pub fn declare_struct_fields(&mut self, id: u32, fields: Vec<kai_tast::KaiType>) {
        if self.struct_fields.len() <= id as usize {
            self.struct_fields.resize(id as usize + 1, Vec::new());
        }
        self.struct_fields[id as usize] = fields;
    }
}
