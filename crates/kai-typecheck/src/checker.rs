//! Shared type-checker state: diagnostics, the local-variable scope stack,
//! and the struct layout table.

use crate::scope::Locals;
use kai_diagnostics::Diagnostic;
use kai_resolver::Resolution;
use kai_tast::{KaiType, StructId};
use crate::sql::snapshot::SqlSnapshot;

/// One struct's resolved layout: fields in declaration order. Field ORDER is
/// ABI — it drives LLVM struct types and getelementptr indices.
#[derive(Debug, Clone)]
pub(crate) struct StructLayout {
    pub name: String,
    pub fields: Vec<FieldSlot>,
}

#[derive(Debug, Clone)]
pub(crate) struct FieldSlot {
    pub name: String,
    pub ty: KaiType,
}

/// Pre-resolved function signature. Bodies are irrelevant for calls, so
/// signatures are collected before any body lowers — recursion and
/// out-of-order references need no fixpoint iteration.
#[derive(Debug, Clone)]
pub(crate) struct FnInfo {
    pub name: String,
    pub param_tys: Vec<KaiType>,
    pub ret: KaiType,
}

pub(crate) struct Checker {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) locals: Locals,
    /// Name tables from resolution: per-module tables hold GLOBAL indices
    /// into the merged program; imports map aliases to module indices.
    pub(crate) resolution: Resolution,
    /// Layouts indexed by `StructId` (= global declaration index).
    pub(crate) structs: Vec<StructLayout>,
    /// Signatures indexed by `FunctionId` (= global declaration index).
    pub(crate) fns: Vec<FnInfo>,
    /// Module currently being lowered: unqualified lookups resolve ONLY
    /// against this module's own tables (§3.6 — imports never leak names).
    pub(crate) current_module: usize,
    /// Display file of the declaration being lowered, stamped onto every
    /// diagnostic so multi-file programs attribute errors correctly (§8.6).
    pub(crate) cur_file: String,
    pub(crate) snapshots: std::collections::HashMap<u32, SqlSnapshot>,
    pub(crate) api_snapshots: std::collections::HashMap<(String, u32), crate::api::snapshot::ApiSnapshot>,
    pub(crate) current_schema: Option<SqlSnapshot>,
}

impl Checker {
    pub fn new(resolution: &Resolution) -> Self {
        Self {
            diagnostics: Vec::new(),
            locals: Locals::new(),
            resolution: resolution.clone(),
            structs: Vec::new(),
            fns: Vec::new(),
            current_module: 0,
            cur_file: String::new(),
            snapshots: std::collections::HashMap::new(),
            api_snapshots: std::collections::HashMap::new(),
            current_schema: None,
        }
    }

    /// Types visible WITHOUT qualification from the module being lowered.
    pub(crate) fn local_types(&self) -> &std::collections::HashMap<String, usize> {
        &self.resolution.module_types[self.current_module]
    }

    /// Functions visible WITHOUT qualification from the module being lowered.
    pub(crate) fn local_fns(&self) -> &std::collections::HashMap<String, usize> {
        &self.resolution.module_fns[self.current_module]
    }

    /// Import table of the module being lowered: alias -> module index.
    pub(crate) fn imports(&self) -> &std::collections::HashMap<String, usize> {
        &self.resolution.imports[self.current_module]
    }

    pub fn error(&mut self, mut diagnostic: Diagnostic) {
        if !self.cur_file.is_empty() {
            diagnostic = diagnostic.with_file(self.cur_file.clone());
        }
        self.diagnostics.push(diagnostic);
    }

    /// Signature clone is small (a few scalars); keeps borrow rules simple.
    pub(crate) fn fn_signature(&self, id: kai_tast::FunctionId) -> FnInfo {
        self.fns[id.0 as usize].clone()
    }

    pub fn failed(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Struct name for diagnostics (`Display` on `KaiType` cannot know it).
    pub(crate) fn type_name(&self, id: StructId) -> &str {
        &self.structs[id.0 as usize].name
    }

    /// Resolved field slot by position-in-layout lookup from a name.
    pub(crate) fn field_slot(&self, id: StructId, field: &str) -> Option<(u16, &FieldSlot)> {
        let layout = &self.structs[id.0 as usize];
        layout
            .fields
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.name == field)
            .map(|(idx, slot)| (idx as u16, slot))
    }
}
