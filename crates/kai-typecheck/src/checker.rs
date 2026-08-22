//! Shared type-checker state: diagnostics, the local-variable scope stack,
//! and the struct layout table.

use crate::scope::Locals;
use kai_diagnostics::Diagnostic;
use kai_resolver::Resolution;
use kai_tast::{KaiType, StructId};

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
    /// Kept for future diagnostics; lookups key off `Resolution.fns`.
    #[allow(dead_code)]
    pub name: String,
    pub param_tys: Vec<KaiType>,
    pub ret: KaiType,
}

pub(crate) struct Checker {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) locals: Locals,
    /// Name tables from resolution (struct/fn name -> declaration index).
    pub(crate) resolution: Resolution,
    /// Layouts indexed by `StructId` (= declaration index).
    pub(crate) structs: Vec<StructLayout>,
    /// Signatures indexed by `FunctionId` (= declaration index).
    pub(crate) fns: Vec<FnInfo>,
}

impl Checker {
    pub fn new(resolution: &Resolution) -> Self {
        Self {
            diagnostics: Vec::new(),
            locals: Locals::new(),
            resolution: resolution.clone(),
            structs: Vec::new(),
            fns: Vec::new(),
        }
    }

    /// Signature clone is small (a few scalars); keeps borrow rules simple.
    pub(crate) fn fn_signature(&self, id: kai_tast::FunctionId) -> FnInfo {
        self.fns[id.0 as usize].clone()
    }

    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
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
