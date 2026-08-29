with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

text = text.replace('for (_m_idx, _module) in modules.iter().enumerate() {', 'for (m_idx, module) in modules.iter().enumerate() {')
text = text.replace('    for (m_idx, module) in modules.iter().enumerate() {\n    }\n', '')

with open('crates/kai-resolver/src/tables.rs', 'w') as f:
    f.write(text)
