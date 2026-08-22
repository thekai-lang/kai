use crate::ident::Ident;
use kai_diagnostics::Span;

/// `use a.b.c;` — module import (v0.0.4). The dotted path resolves to
/// `a/b/c.kai` under the project root; the LAST segment becomes the alias
/// every qualified reference goes through (`c.member`). Imports never
/// inject into scope — resolution is always alias-qualified (§3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub path: Vec<Ident>,
    pub span: Span,
}
