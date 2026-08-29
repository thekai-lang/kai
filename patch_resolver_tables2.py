import re
with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

old_fn_loop = """        for decl in &module.program.fns {
            let global = resolution.fn_module.len();
            if resolution.module_fns[m_idx]
                .insert(decl.name.name.clone(), global)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate function `{}`", decl.name.name),
                        decl.name.span,
                    )
                    .with_file(module.file),
                );
            }
            resolution.fn_is_public.push(decl.is_public);
            resolution.fn_module.push(m_idx);
            resolution.fn_file.push(module.file.to_string());
        }"""

new_fn_loop = """        for decl in &module.program.fns {
            let global = resolution.fn_module.len();
            let name_str = decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
            let span = kai_diagnostics::Span::merge(decl.path.first().unwrap().span, decl.path.last().unwrap().span);
            
            // Ownership validation
            if decl.path.len() >= 2 {
                let owner_type = &decl.path[0].name;
                if !resolution.module_types[m_idx].contains_key(owner_type) {
                    diagnostics.push(
                        Diagnostic::error(
                            format!("associated function `{}` must be defined in the same module that owns the type `{}`", name_str, owner_type),
                            span,
                        )
                        .with_file(module.file),
                    );
                }
            }
            
            if resolution.module_fns[m_idx]
                .insert(name_str.clone(), global)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate function `{}`", name_str),
                        span,
                    )
                    .with_file(module.file),
                );
            }
            resolution.fn_is_public.push(decl.is_public);
            resolution.fn_module.push(m_idx);
            resolution.fn_file.push(module.file.to_string());
        }"""

text = text.replace(old_fn_loop, new_fn_loop)

# Ty::Named
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*table\.get\(&\1\.name\)\.copied\(\)\.into_iter\(\),',
    r'Ty::Path(path) => if path.len() == 1 { table.get(&path[0].name).copied() } else { None }.into_iter(),',
    text
)
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*match\s*resolution\.module_types\[module_id\]\n\s*\.get\(&\1\.name\)',
    r'Ty::Path(path) => match if path.len() == 1 { resolution.module_types[module_id].get(&path[0].name) } else { None }',
    text
)
# wait, the exact string match might fail.
