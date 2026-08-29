with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if 'decl.name.name' in line:
        lines[i] = line.replace('decl.name.name', 'decl.path.last().unwrap().name')
    if 'f.name.span' in line:
        lines[i] = line.replace('f.name.span', 'f.path.last().unwrap().span')
    if 'main.name.span' in line:
        lines[i] = line.replace('main.name.span', 'main.path.last().unwrap().span')
    if 'Ty::Named(ident) if ident.name' in line:
        lines[i] = line.replace('Ty::Named(ident) if ident.name == "int32" || ident.name == "int"', 'Ty::Path(path) if path.len() == 1 && (path[0].name == "int32" || path[0].name == "int")')
        
    if 'Ty::Named(Ident {' in line:
        lines[i] = line.replace('Ty::Named(Ident {', 'Ty::Path(vec![Ident {')
        lines[i+3] = lines[i+3].replace('})', '}])')
        
    if 'name: Ident {' in line and 'main' in lines[i+1]:
        lines[i] = line.replace('name: Ident {', 'path: vec![Ident {')
        lines[i+3] = lines[i+3].replace('},', '}],')

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.writelines(lines)
