use std::collections::HashMap;
use std::path::PathBuf;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

use kai_diagnostics::Span;

/// Per-module source metadata for runtime panic locations (§10.1): the
/// display path baked into `at file:line:col`, plus line-start offsets.
///
/// v0.22: `text` is retained — `require`/`observe` condition text (§5.2) is
/// the raw source-text span, sliceable only from the original source. The
/// text already arrives via `SourceUnit` and outlives codegen, so this
/// extends no lifetime.
pub(crate) struct SourceInfo {
    pub file: String,
    /// Byte offset where each 1-based line starts; entry 0 is always 0.
    pub line_starts: Vec<u32>,
    /// Full module source — span slicing for §5.2 condition text.
    pub text: String,
}

impl SourceInfo {
    pub fn new(file: &str, text: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (idx, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx as u32 + 1);
            }
        }
        Self {
            file: file.to_string(),
            line_starts,
            text: text.to_string(),
        }
    }

    /// Raw source-text span (v0.22): the condition text exactly as written,
    /// verbatim including embedded newlines if any.
    pub fn slice(&self, span: Span) -> String {
        let start = span.start.min(self.text.len());
        let end = span.end.min(self.text.len()).max(start);
        self.text[start..end].to_string()
    }

    /// 1-based (line, column) of a byte offset; offsets past the end clamp
    /// to the final position.
    pub fn line_col(&self, offset: usize) -> (i64, i64) {
        let offset = (offset as u32).min(*self.line_starts.last().unwrap_or(&0));
        let idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        (
            (idx + 1) as i64,
            (offset - self.line_starts[idx] + 1) as i64,
        )
    }

    /// `"file:line:col"` — the location field of §5.2.2 records and §10.3.
    pub fn location(&self, span: Span) -> String {
        let (line, col) = self.line_col(span.start);
        format!("{}:{line}:{col}", self.file)
    }
}

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
    /// Source info keyed by dotted module name (`""` = entry module).
    pub sources: HashMap<String, SourceInfo>,
    /// Lazily generated ownership helpers, keyed by type stem. RefCell
    /// because helper generation recurses while other code holds &Ctx.
    pub retain_helpers: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    pub release_helpers: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    pub elem_dtors: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    /// Cached per-type reversible snapshot release thunks (§5.3): a function
    /// `void @kai.snapREL.<T>(ptr value)` that releases one value of `T`, used
    /// by the runtime ledger to drop snapshot/current heap claims generically.
    pub snapshot_dtors: std::cell::RefCell<HashMap<String, FunctionValue<'ctx>>>,
    /// Module-file name globals baked for panic sites, one per module key.
    pub file_globals: std::cell::RefCell<HashMap<String, inkwell::values::PointerValue<'ctx>>>,
    /// Monotonic counter for per-literal closure artifacts (body functions,
    /// environment dtors) — each literal is distinct code.
    pub closure_seq: std::cell::Cell<u32>,
    /// §5.2.2/v0.21: project root for the `.kai/observe.log` / `.kai/debt.log`
    /// sinks. `None` = string API (documented recording no-op, never CWD).
    pub sink_root: Option<PathBuf>,
    /// `true` while emitting a `reversible` function body (§5.3): runtime
    /// panic blocks emit `kai_reversible_unwind` before §10.1 `kai_panic`.
    /// Reset to `false` for ordinary/helper emission so golden IR stays stable.
    pub reversible_active: std::cell::Cell<bool>,
}

impl<'ctx> Ctx<'ctx> {
    pub fn new(
        context: &'ctx Context,
        module_name: &str,
        sources: HashMap<String, SourceInfo>,
        sink_root: Option<PathBuf>,
    ) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            structs: Vec::new(),
            struct_fields: Vec::new(),
            functions: Vec::new(),
            sources,
            retain_helpers: Default::default(),
            release_helpers: Default::default(),
            elem_dtors: Default::default(),
            snapshot_dtors: Default::default(),
            file_globals: Default::default(),
            closure_seq: std::cell::Cell::new(0),
            sink_root,
            reversible_active: std::cell::Cell::new(false),
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
