import re
with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = re.sub(
    r'Ty::Named\(ident\)\s*if\s*ident\.name\s*==\s*"int32"\s*\|\|\s*ident\.name\s*==\s*"unit"',
    r'Ty::Path(path) if path.len() == 1 && (path[0].name == "int32" || path[0].name == "unit")',
    text
)

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)
