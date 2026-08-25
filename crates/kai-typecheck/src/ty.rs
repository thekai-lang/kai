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
            "string" => KaiType::String,
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
        // v0.0.6 (§9.9a): tagged unions and closures resolve structurally;
        // there is no nominal generic table — the builtin names desugared
        // at parse time and unknown inner types report per use.
        Ty::Optional(inner) => KaiType::Optional(Box::new(resolve(checker, inner))),
        Ty::Result { ok, err } => KaiType::Result {
            ok: Box::new(resolve(checker, ok)),
            err: Box::new(resolve(checker, err)),
        },
        Ty::Closure { params, ret } => KaiType::Closure {
            params: params.iter().map(|p| resolve(checker, p)).collect(),
            ret: Box::new(resolve(checker, ret)),
        },
        Ty::Temporal { inner, origin, duration } => {
            if duration.value == 0 {
                checker.error(error::temporal_zero_duration(duration.span));
            }
            let inner_ty = resolve(checker, inner);
            let unit = match duration.unit {
                kai_ast::DurationUnit::Ms => kai_tast::DurationUnit::Ms,
                kai_ast::DurationUnit::S => kai_tast::DurationUnit::S,
                kai_ast::DurationUnit::M => kai_tast::DurationUnit::M,
                kai_ast::DurationUnit::H => kai_tast::DurationUnit::H,
                kai_ast::DurationUnit::D => kai_tast::DurationUnit::D,
            };
            let origin_ty = match origin {
                kai_ast::TemporalOrigin::Local => kai_tast::TemporalOrigin::Local,
                kai_ast::TemporalOrigin::Wallclock => kai_tast::TemporalOrigin::Wallclock,
            };
            KaiType::Temporal {
                inner: Box::new(inner_ty),
                origin: origin_ty,
                duration: kai_tast::DurationLit {
                    value: duration.value,
                    unit,
                },
            }
        }
    }
}
