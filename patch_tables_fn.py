import re
with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

# Only replace inside the loop for module.program.fns
idx = text.find('for decl in &module.program.fns {')
idx_end = text.find('resolution.fn_is_public.push(decl.is_public);', idx)
idx_end = text.find('}', idx_end)

if idx != -1 and idx_end != -1:
    old_loop = text[idx:idx_end+1]
    new_loop = old_loop.replace('decl.name.name.clone()', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')
    new_loop = new_loop.replace('decl.name.name', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')
    new_loop = new_loop.replace('decl.name.span', 'kai_diagnostics::Span::merge(decl.path.first().unwrap().span, decl.path.last().unwrap().span)')
    
    # Insert ownership validation
    # right after let global = resolution.fn_module.len();
    validation = """
            let name_str = decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
            let span = kai_diagnostics::Span::merge(decl.path.first().unwrap().span, decl.path.last().unwrap().span);
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
"""
    new_loop = new_loop.replace('let global = resolution.fn_module.len();', 'let global = resolution.fn_module.len();\n' + validation)
    text = text[:idx] + new_loop + text[idx_end+1:]

text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*table\.get\(&\1\.name\)\.copied\(\)\.into_iter\(\)\.collect\(\),',
    r'Ty::Path(path) => if path.len() == 1 { table.get(&path[0].name).copied().into_iter().collect() } else { vec![] },',
    text
)
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*match\s*resolution\.module_types\[module_idx\]\.get\(&\1\.name\)',
    r'Ty::Path(path) => match if path.len() == 1 { resolution.module_types[module_idx].get(&path[0].name) } else { None }',
    text
)

with open('crates/kai-resolver/src/tables.rs', 'w') as f:
    f.write(text)
