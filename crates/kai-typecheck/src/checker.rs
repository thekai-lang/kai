//! Shared type-checker state: diagnostics + the local-variable scope stack.

use crate::scope::Locals;
use kai_diagnostics::Diagnostic;

pub(crate) struct Checker {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) locals: Locals,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            locals: Locals::new(),
        }
    }

    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn failed(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}
