import re
with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

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
