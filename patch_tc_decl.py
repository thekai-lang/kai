import re
with open('crates/kai-typecheck/src/decl.rs', 'r') as f:
    text = f.read()

text = text.replace('decl.name.name.clone()', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')
with open('crates/kai-typecheck/src/decl.rs', 'w') as f:
    f.write(text)
