import re

with open('crates/kai-typecheck/src/lib.rs', 'r') as f:
    text = f.read()
text = text.replace('mod v0012_tests;', '#[cfg(test)]\nmod v0012_tests;')
with open('crates/kai-typecheck/src/lib.rs', 'w') as f:
    f.write(text)

with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()
text = re.sub(r'name: Ident \{\n\s*name: "main"\.to_string\(\),\n\s*span: Span::new\(0, 0\),\n\s*\},', r'path: vec![Ident { name: "main".to_string(), span: Span::new(0, 0) }],', text)
with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)

with open('crates/kai-parser/src/v0003_tests.rs', 'r') as f:
    text = f.read()
text = text.replace('root.last().unwrap().name', 'root.name')
with open('crates/kai-parser/src/v0003_tests.rs', 'w') as f:
    f.write(text)

with open('crates/kai-parser/src/v0005_surface_tests.rs', 'r') as f:
    text = f.read()
text = text.replace('root.last().unwrap().name', 'root.name')
with open('crates/kai-parser/src/v0005_surface_tests.rs', 'w') as f:
    f.write(text)

