import re
with open('crates/kai-driver/src/modules.rs', 'r') as f:
    text = f.read()

old_logic = """            let path = self.root.join(&expected);
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    self.load_module(&target, expected, source)?;
                }
                Err(_) => {
                    return Err(vec![
                        Diagnostic::error(format!("cannot find module `{target}`"), decl.span)
                            .with_file(importer_file),
                    ]);
                }
            }"""

new_logic = """            let path = self.root.join(&expected);
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    self.load_module(&target, expected, source)?;
                }
                Err(_) => {
                    // Try direct symbol import fallback: maybe the last segment is a symbol, not a module
                    if decl.path.len() >= 2 {
                        let parent_target = decl.path[..decl.path.len()-1].iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
                        let parent_expected = format!("{}.kai", parent_target.replace('.', "/"));
                        let parent_path = self.root.join(&parent_expected);
                        if let Ok(parent_source) = std::fs::read_to_string(&parent_path) {
                            self.load_module(&parent_target, parent_expected, parent_source)?;
                            continue;
                        }
                    }
                    
                    return Err(vec![
                        Diagnostic::error(format!("cannot find module or symbol `{target}`"), decl.span)
                            .with_file(importer_file),
                    ]);
                }
            }"""

text = text.replace(old_logic, new_logic)

with open('crates/kai-driver/src/modules.rs', 'w') as f:
    f.write(text)
