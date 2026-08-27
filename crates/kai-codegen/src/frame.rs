//! Per-function emission state: the table mapping TAST local ids to their
//! stack slots. Ids are unique per function (type checker guarantee), so a
//! flat map suffices; scoping rules were fully applied upstream.

use std::collections::HashMap;

use inkwell::values::PointerValue;
use kai_tast::LocalId;

pub(crate) struct Frame<'ctx> {
    /// Dotted module name of the function under construction (`""` =
    /// entry); selects the source that panic locations resolve against.
    pub module: String,
    /// `true` inside a `reversible` function (§5.3): return sites emit
    /// `kai_reversible_commit` to release the activation's snapshot claims.
    pub reversible: bool,
    slots: HashMap<u32, PointerValue<'ctx>>,
}

impl<'ctx> Frame<'ctx> {
    pub fn new(module: String) -> Self {
        Self {
            module,
            reversible: false,
            slots: HashMap::new(),
        }
    }

    pub fn bind(&mut self, local: LocalId, slot: PointerValue<'ctx>) {
        self.slots.insert(local.0, slot);
    }

    pub fn slot(&self, local: LocalId) -> PointerValue<'ctx> {
        self.slots[&local.0]
    }
}
