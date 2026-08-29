import re
with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

old_import_check = """            let Some(&target_idx) = name_to_index.get(target.as_str()) else {
                diagnostics.push(
                    Diagnostic::error(
                        format!("cannot find module `{target}`"),
                        decl.span,
                    )
                    .with_file(module.file),
                );
                continue;
            };
            if target_idx == m_idx {
                diagnostics.push(
                    Diagnostic::error(
                        format!("cyclic import: {target} -> {target}"),
                        decl.span,
                    )
                    .with_file(module.file),
                );
                continue;
            }

            if resolution.imports[m_idx]
                .insert(alias.name.clone(), target_idx)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate import alias `{}`", alias.name),
                        alias.span,
                    )
                    .with_file(module.file),
                );
            }"""

new_import_check = """            if let Some(&target_idx) = name_to_index.get(target.as_str()) {
                if target_idx == m_idx {
                    diagnostics.push(
                        Diagnostic::error(
                            format!("cyclic import: {target} -> {target}"),
                            decl.span,
                        )
                        .with_file(module.file),
                    );
                    continue;
                }

                if resolution.imports[m_idx]
                    .insert(alias.name.clone(), target_idx)
                    .is_some()
                {
                    diagnostics.push(
                        Diagnostic::error(
                            format!("duplicate import alias `{}`", alias.name),
                            alias.span,
                        )
                        .with_file(module.file),
                    );
                }
            } else {
                // Not a direct module match. Check for direct symbol import.
                if decl.path.len() >= 2 {
                    let parent = decl.path[..decl.path.len()-1].iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
                    let symbol = &decl.path.last().unwrap().name;
                    if let Some(&target_idx) = name_to_index.get(parent.as_str()) {
                        let mut found = false;
                        if let Some(&global_idx) = resolution.module_types[target_idx].get(symbol) {
                            if !resolution.type_is_public[global_idx] {
                                diagnostics.push(Diagnostic::error(format!("type `{symbol}` is private"), decl.span).with_file(module.file));
                            } else {
                                resolution.module_types[m_idx].insert(symbol.clone(), global_idx);
                                found = true;
                            }
                        }
                        if let Some(&global_idx) = resolution.module_fns[target_idx].get(symbol) {
                            if !resolution.fn_is_public[global_idx] {
                                diagnostics.push(Diagnostic::error(format!("function `{symbol}` is private"), decl.span).with_file(module.file));
                            } else {
                                resolution.module_fns[m_idx].insert(symbol.clone(), global_idx);
                                found = true;
                            }
                        }
                        if !found {
                            diagnostics.push(Diagnostic::error(format!("cannot find module or symbol `{target}`"), decl.span).with_file(module.file));
                        }
                        continue;
                    }
                }
                diagnostics.push(
                    Diagnostic::error(
                        format!("cannot find module `{target}`"),
                        decl.span,
                    )
                    .with_file(module.file),
                );
            }"""

text = text.replace(old_import_check, new_import_check)

with open('crates/kai-resolver/src/tables.rs', 'w') as f:
    f.write(text)
