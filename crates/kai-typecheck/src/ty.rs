//! Surface type names -> concrete `KaiType`. Aliases per §3.2: `int` = int32,
//! `float` = float64; declared structs resolve nominally via the resolution
//! tables.

use crate::checker::Checker;
use crate::error;
use kai_ast::Ty;
use kai_tast::{KaiType, StructId};

pub(crate) fn resolve(checker: &mut Checker, ty: &Ty) -> KaiType {
    match ty {
        Ty::Path(path) => {
            if path.len() == 1 {
                let name = path[0].name.as_str();
                match name {
                    "int32" | "int" => KaiType::Int32,
                    "int64" => KaiType::Int64,
                    "float64" | "float" => KaiType::Float64,
                    "bool" => KaiType::Bool,
                    "string" => KaiType::String,
                    "unit" => KaiType::Unit,
                    other => match checker.local_types().get(other) {
                        Some(&idx) => KaiType::Struct(StructId(idx as u32)),
                        None => {
                            let span = path[0].span;
                            checker.error(error::unknown_type(other, span));
                            KaiType::Int32
                        }
                    },
                }
            } else if path.len() == 2 {
                let alias = path[0].name.as_str();
                let type_name = path[1].name.as_str();
                let m_idx = checker.current_module;
                
                if let Some(&target_mod) = checker.resolution.imports[m_idx].get(alias) {
                    if let Some(&idx) = checker.resolution.module_types[target_mod].get(type_name) {
                        if checker.resolution.type_is_public[idx] {
                            KaiType::Struct(StructId(idx as u32))
                        } else {
                            checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("type `{type_name}` is private"), path[1].span).with_file(&checker.cur_file));
                            KaiType::Int32
                        }
                    } else {
                        checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("module `{alias}` has no type `{type_name}`"), path[1].span).with_file(&checker.cur_file));
                        KaiType::Int32
                    }
                } else {
                    checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("unknown module alias `{alias}`"), path[0].span).with_file(&checker.cur_file));
                    KaiType::Int32
                }
            } else {
                let span = path[0].span; // simplified
                checker.diagnostics.push(kai_diagnostics::Diagnostic::error("invalid type path length".to_string(), span).with_file(&checker.cur_file));
                KaiType::Int32
            }
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
