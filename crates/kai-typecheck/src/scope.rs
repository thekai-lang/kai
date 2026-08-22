//! Function-local variable scopes. A stack of hash maps; nested blocks push a
//! new scope, so shadowing across scopes works while same-scope redeclaration
//! is an error.

use std::collections::HashMap;

use kai_tast::{KaiType, LocalId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalInfo {
    pub id: LocalId,
    pub ty: KaiType,
    /// `true` when declared with `var`; assignment to immutable bindings is
    /// rejected by the type checker (§9.3, v0.0.2 exit criteria).
    pub mutable: bool,
}

pub struct Locals {
    scopes: Vec<HashMap<String, LocalInfo>>,
    next_id: u32,
}

impl Locals {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_id: 0,
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

    /// Declares a binding in the current scope. Returns `None` when the name
    /// already exists there (the caller reports the diagnostic; the id counter
    /// still advances so ids stay unique).
    pub fn declare(&mut self, name: &str, ty: KaiType, mutable: bool) -> Option<LocalInfo> {
        let id = LocalId(self.next_id);
        self.next_id += 1;
        let info = LocalInfo { id, ty, mutable };

        let current = self.scopes.last_mut().expect("scope stack never empty");
        if current.contains_key(name) {
            return None;
        }
        current.insert(name.to_owned(), info);
        Some(info)
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
        let info = locals.declare("x", KaiType::Int32, false).unwrap();
        assert_eq!(locals.lookup("x").unwrap().id, info.id);
    }

    #[test]
    fn same_scope_redeclaration_fails() {
        let mut locals = Locals::new();
        locals.declare("x", KaiType::Int32, false);
        assert!(locals.declare("x", KaiType::Int64, true).is_none());
    }

    #[test]
    fn nested_scope_shadows_and_unshadows() {
        let mut locals = Locals::new();
        let outer = locals.declare("x", KaiType::Int32, false).unwrap();

        locals.push_scope();
        let inner = locals.declare("x", KaiType::Float64, true).unwrap();
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
