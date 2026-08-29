import re
with open('crates/kai-typecheck/src/ty.rs', 'r') as f:
    text = f.read()

old_resolve = """pub(crate) fn resolve(checker: &mut Checker, ty: &Ty) -> KaiType {
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
        },"""

new_resolve = """pub(crate) fn resolve(checker: &mut Checker, ty: &Ty) -> KaiType {
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
                let m_idx = checker.ctx.module_id;
                
                if let Some(&target_mod) = checker.ctx.resolution.imports[m_idx].get(alias) {
                    if let Some(&idx) = checker.ctx.resolution.module_types[target_mod].get(type_name) {
                        if checker.ctx.resolution.type_is_public[idx] {
                            KaiType::Struct(StructId(idx as u32))
                        } else {
                            checker.error(error::custom(format!("type `{type_name}` is private"), path[1].span));
                            KaiType::Int32
                        }
                    } else {
                        checker.error(error::custom(format!("module `{alias}` has no type `{type_name}`"), path[1].span));
                        KaiType::Int32
                    }
                } else {
                    checker.error(error::custom(format!("unknown module alias `{alias}`"), path[0].span));
                    KaiType::Int32
                }
            } else {
                let span = path[0].span; // simplified
                checker.error(error::custom("invalid type path length", span));
                KaiType::Int32
            }
        },"""

text = text.replace(old_resolve, new_resolve)

with open('crates/kai-typecheck/src/ty.rs', 'w') as f:
    f.write(text)
