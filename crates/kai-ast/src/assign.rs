//! Assignment statements. Assignment is statement-only (EBNF §6, v0.0.2
//! decision): no `a = b = c` chains.

use crate::ident::Ident;
use kai_diagnostics::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
}

/// The place being written to. Identifiers only in v0.0.2; field/index
/// places arrive with structs and arrays (v0.0.3+).
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Named(Ident),
}

impl AssignTarget {
    pub fn span(&self) -> Span {
        match self {
            AssignTarget::Named(ident) => ident.span,
        }
    }
}
