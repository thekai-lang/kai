//! Shared type-checker state: diagnostics + the local-variable scope stack.

use crate::scope::Locals;
use kai_diagnostics::Diagnostic;
use kai_resolver::Resolution;

pub(crate) struct Checker {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) locals: Locals,
    /// Name tables from resolution (struct/fn name -> declaration index).
    /// Consumed fully once struct typing lands later in v0.0.3.
    #[allow(dead_code)]
    pub(crate) resolution: Resolution,
}

impl Checker {
    pub fn new(resolution: &Resolution) -> Self {
        Self {
            diagnostics: Vec::new(),
            locals: Locals::new(),
            resolution: resolution.clone(),
        }
    }

    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn failed(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}
