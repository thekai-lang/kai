use crate::ident::Ident;
use kai_diagnostics::Span;

/// `use a.b.c;` — module import (v0.0.4). The dotted path resolves to
/// `a/b/c.kai` under the project root; the LAST segment becomes the alias
/// every qualified reference goes through (`c.member`). Imports never
/// inject into scope — resolution is always alias-qualified (§3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub path: Vec<Ident>,
    pub as_alias: Option<Ident>,
    pub span: Span,
}

impl UseDecl {
    /// Dotted name (`a.b.c`) for diagnostics and module lookup.
    /// Centralizes the `join(".")` previously duplicated in
    /// `kai-driver` and `kai-resolver`.
    pub fn dotted_name(&self) -> String {
        self.path
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Alias is the last segment (`c` in `a.b.c`).
    pub fn alias(&self) -> Option<&Ident> {
        self.as_alias.as_ref().or_else(|| self.path.last())
    }
}
