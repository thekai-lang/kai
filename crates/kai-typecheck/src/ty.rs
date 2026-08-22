use crate::error;
use kai_ast::Ty;
use kai_diagnostics::{Diagnostic, Span};
use kai_tast::KaiType;

/// Surface name -> concrete type. `int` is an alias for `int32` (§3.2).
pub fn resolve(ty: &Ty, span: Span, diagnostics: &mut Vec<Diagnostic>) -> KaiType {
    match ty {
        Ty::Named(ident) => match ident.name.as_str() {
            "int32" | "int" => KaiType::Int32,
            other => {
                diagnostics.push(error::unknown_type(other, span));
                KaiType::Int32 // placeholder; program is discarded on error anyway
            }
        },
    }
}
