import os
import re

def fix_file(filepath):
    if not os.path.exists(filepath):
        return
    with open(filepath, 'r') as f:
        text = f.read()

    text = text.replace('Ty::Named(', 'Ty::Path(')
    text = re.sub(r'ident\.name,\s*"(.*?)"', r'ident.last().unwrap().name, "\1"', text)
    text = re.sub(r'n\.name\s*==\s*"(.*?)"', r'n.last().unwrap().name == "\1"', text)
    text = re.sub(r't\.name,\s*"(.*?)"', r't.last().unwrap().name, "\1"', text)
    text = text.replace('main.name.name, "main"', 'main.path.last().unwrap().name, "main"')
    
    with open(filepath, 'w') as f:
        f.write(text)

files_to_fix = [
    'crates/kai-parser/src/tests.rs',
    'crates/kai-parser/src/v0003_tests.rs',
    'crates/kai-parser/src/v0005_surface_tests.rs',
    'crates/kai-parser/src/v0006_tests.rs'
]

for fp in files_to_fix:
    fix_file(fp)

# Fix kai-resolver tests
with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = text.replace('Ty::Named(Ident {', 'Ty::Path(vec![Ident {')
text = text.replace('name: "int32".to_string(),\n                    span: Span::new(0, 0),\n                })', 'name: "int32".to_string(),\n                    span: Span::new(0, 0),\n                }])')
text = text.replace('name: Ident {', 'path: vec![Ident {')
text = text.replace('name: "main".to_string(),\n                span: Span::new(0, 0),\n            },', 'name: "main".to_string(),\n                span: Span::new(0, 0),\n            }],')

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)

