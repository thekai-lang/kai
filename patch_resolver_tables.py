import re

with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

# For FnDecl's name, we need to join the path with "."
text = text.replace(
    'decl.name.name.clone()',
    'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")'
)
text = text.replace(
    'decl.name.name',
    'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")'
)
text = text.replace(
    'decl.name.span',
    'kai_diagnostics::Span::merge(decl.path.first().unwrap().span, decl.path.last().unwrap().span)'
)
# For Ty::Named -> Ty::Path
# 284 |                 Ty::Named(ident) => table.get(&ident.name).copied().into_iter(),
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*table\.get\(&\1\.name\)',
    r'Ty::Path(path) => if path.len() == 1 { table.get(&path[0].name) } else { None }',
    text
)
# 416 |             Ty::Named(ident) => match resolution.module_types[module_id]...
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*match\s*resolution\.module_types\[module_id\]\s*\.get\(&\1\.name\)',
    r'Ty::Path(path) => match if path.len() == 1 { resolution.module_types[module_id].get(&path[0].name) } else { None }',
    text
)

# BUT WAIT! The user specified ownership validation!
# "Jika path.len() == 2 (misal ["User", "create"]), pastikan "User" terdaftar di module_types modul asal sebelum meloloskan pendaftaran."
# Let's see how populate is implemented.
