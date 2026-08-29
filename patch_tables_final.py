with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

# Replace all decl.name usages
text = text.replace('decl.name.name.clone()', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')
text = text.replace('decl.name.name', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')
text = text.replace('decl.name.span', 'kai_diagnostics::Span::merge(decl.path.first().unwrap().span, decl.path.last().unwrap().span)')

# Fix Ty::Named usages if they still exist
import re
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
