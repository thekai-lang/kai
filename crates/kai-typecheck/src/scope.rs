//! Function-local variable scopes. A stack of hash maps; nested blocks push a
//! new scope, so shadowing across scopes works while same-scope redeclaration
//! is an error.

use std::collections::HashMap;

use kai_tast::{KaiType, LocalId};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalInfo {
    pub id: LocalId,
    pub ty: KaiType,
    /// `true` when declared with `var`; assignment to immutable bindings is
    /// rejected by the type checker (§9.3, v0.0.2 exit criteria).
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclareOutcome {
    /// Name was new in this scope.
    Fresh(LocalInfo),
    /// Name already bound here; carries the ORIGINAL binding so every
    /// reference keeps resolving to the first declaration. The caller
    /// reports the diagnostic.
    Duplicate(LocalInfo),
}

pub struct Locals {
    scopes: Vec<HashMap<String, LocalInfo>>,
    next_id: u32,
    /// All bindings ever declared, by id — lets the closure-capture analysis
    /// recover a referenced id's type without name lookups.
    by_id: Vec<LocalInfo>,
}

impl Locals {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_id: 0,
            by_id: Vec::new(),
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        // The function-root scope never pops.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Declares a binding in the current scope. Ids only advance for fresh
    /// bindings; duplicates resolve to the original id.
    pub fn declare(&mut self, name: &str, ty: KaiType, mutable: bool) -> DeclareOutcome {
        let current = self
            .scopes
            .last_mut()
            .expect("internal error: scope stack empty — compiler bug, not user code");
        if let Some(existing) = current.get(name).cloned() {
            return DeclareOutcome::Duplicate(existing);
        }

        let info = LocalInfo {
            id: LocalId(self.next_id),
            ty,
            mutable,
        };
        self.next_id += 1;
        self.by_id.push(info.clone());
        current.insert(name.to_owned(), info.clone());
        DeclareOutcome::Fresh(info)
    }

    /// The id the NEXT declaration would receive — a closure body records
    /// this before lowering; any referenced id below it is an outer capture.
    pub(crate) fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Binding info for a previously declared id (capture analysis).
    pub(crate) fn info_of(&self, id: LocalId) -> Option<&LocalInfo> {
        self.by_id.get(id.0 as usize)
    }

    /// Declared name of a binding id — diagnostics only (§9.10 capture
    /// rejection names the offending variable).
    pub(crate) fn name_of(&self, id: LocalId) -> String {
        self.by_id
            .get(id.0 as usize)
            .map(|_| {
                self.scopes
                    .iter()
                    .find_map(|scope| {
                        scope
                            .iter()
                            .find(|(_, info)| info.id == id)
                            .map(|(name, _)| name.clone())
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    /// Innermost binding visible from the current point, if any.
    pub fn lookup(&self, name: &str) -> Option<LocalInfo> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_tast::KaiType;

    #[test]
    fn declares_and_looks_up() {
        let mut locals = Locals::new();
        let DeclareOutcome::Fresh(info) = locals.declare("x", KaiType::Int32, false) else {
            panic!("first declaration must be fresh");
        };
        assert_eq!(locals.lookup("x").unwrap().id, info.id);
    }

    #[test]
    fn duplicate_resolves_to_original_id() {
        let mut locals = Locals::new();
        let DeclareOutcome::Fresh(first) = locals.declare("x", KaiType::Int32, false) else {
            panic!("first declaration must be fresh");
        };

        match locals.declare("x", KaiType::Int64, true) {
            DeclareOutcome::Duplicate(info) => assert_eq!(info.id, first.id),
            DeclareOutcome::Fresh(_) => panic!("redeclaration must be Duplicate"),
        }

        // The counter did not advance: a later fresh name gets the next id.
        let DeclareOutcome::Fresh(next) = locals.declare("y", KaiType::Bool, false) else {
            panic!("distinct name must be fresh");
        };
        assert_eq!(next.id.0, first.id.0 + 1);
    }

    #[test]
    fn nested_scope_shadows_and_unshadows() {
        let mut locals = Locals::new();
        let DeclareOutcome::Fresh(outer) = locals.declare("x", KaiType::Int32, false) else {
            panic!("outer must be fresh");
        };

        locals.push_scope();
        // Shadowing in a nested scope is a genuinely new binding.
        let DeclareOutcome::Fresh(inner) = locals.declare("x", KaiType::Float64, true) else {
            panic!("shadowing must be fresh");
        };
        assert_eq!(locals.lookup("x").unwrap(), inner);

        locals.pop_scope();
        assert_eq!(locals.lookup("x").unwrap(), outer);
    }

    #[test]
    fn root_scope_never_pops() {
        let mut locals = Locals::new();
        locals.pop_scope();
        locals.declare("y", KaiType::Bool, false);
        assert!(locals.lookup("y").is_some());
    }
}
