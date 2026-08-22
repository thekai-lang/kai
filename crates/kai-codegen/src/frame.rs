//! Per-function emission state: the table mapping TAST local ids to their
//! stack slots. Ids are unique per function (type checker guarantee), so a
//! flat map suffices; scoping rules were fully applied upstream.

use std::collections::HashMap;

use inkwell::values::PointerValue;
use kai_tast::LocalId;

pub(crate) struct Frame<'ctx> {
    slots: HashMap<u32, PointerValue<'ctx>>,
}

impl<'ctx> Frame<'ctx> {
    pub fn new() -> Self {
        Self {
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
