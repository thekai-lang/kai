//! Surface type names -> concrete `KaiType`. Aliases per §3.2: `int` = int32,
//! `float` = float64; declared structs resolve nominally via the resolution
//! tables.

use crate::checker::Checker;
use crate::error;
use kai_ast::Ty;
use kai_tast::{KaiType, StructId};

pub(crate) fn resolve(checker: &mut Checker, ty: &Ty) -> KaiType {
    match ty {
        Ty::Named(ident) => match ident.name.as_str() {
            "int32" | "int" => KaiType::Int32,
            "int64" => KaiType::Int64,
            "float64" | "float" => KaiType::Float64,
            "bool" => KaiType::Bool,
            "unit" => KaiType::Unit,
            other => match checker.local_types().get(other) {
                Some(&idx) => KaiType::Struct(StructId(idx as u32)),
                None => {
                    let span = ident.span;
                    checker.error(error::unknown_type(other, span));
                    KaiType::Int32 // placeholder; program is discarded on error anyway
                }
            },
        },
        // `T[]`: the element type resolves like any other reference.
        Ty::Array(elem) => KaiType::Array(Box::new(resolve(checker, elem))),
    }
}
