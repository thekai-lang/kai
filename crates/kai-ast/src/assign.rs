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

/// The place being written to (EBNF `Place ::= Ident | Place '.' Ident`).
/// A bare identifier in v0.0.2; field paths from v0.0.3. Array-index places
/// arrive with arrays (v0.0.5).
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Named(Ident),
    /// `root.field1.field2` — the write goes through a field path rooted at
    /// a local binding; mutability of the root gates the whole place.
    Field {
        root: Ident,
        path: Vec<Ident>,
    },
}

impl AssignTarget {
    pub fn span(&self) -> Span {
        match self {
            AssignTarget::Named(ident) => ident.span,
            AssignTarget::Field { root, path } => path
                .last()
                .map_or(root.span, |last| Span::merge(root.span, last.span)),
        }
    }
}
