//! Assignment statements. Assignment is statement-only (EBNF §6, v0.0.2
//! decision): no `a = b = c` chains.
//!
//! v0.0.5 generalizes the target to the full `Place` grammar (EBNF §6):
//! one root identifier plus any mix of field and index projections.
//! Writability is a property of the ROOT binding alone (`var`/`mut` param);
//! every projection uniformly inherits it (§9.3's two-axis model).

use crate::expr::Expr;
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

/// One projection step off a place root.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceStep {
    Field(Ident),
    Index {
        index: Expr,
        /// Closing bracket span, for targeted diagnostics.
        rbracket: Span,
    },
}

impl PlaceStep {
    pub fn span(&self) -> Span {
        match self {
            PlaceStep::Field(ident) => ident.span,
            PlaceStep::Index { index, rbracket } => Span::merge(index.span, *rbracket),
        }
    }
}

/// The place being written to. `root` is found by stripping every
/// projection down to the base identifier; its mutability gates the write.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Named(Ident),
    Path {
        root: Ident,
        steps: Vec<PlaceStep>,
    },
}

impl AssignTarget {
    pub fn span(&self) -> Span {
        match self {
            AssignTarget::Named(ident) => ident.span,
            AssignTarget::Path { root, steps } => steps
                .last()
                .map_or(root.span, |last| Span::merge(root.span, last.span())),
        }
    }
}
