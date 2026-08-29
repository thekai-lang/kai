with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if 'name: Ident {' in line and 'main' in lines[i+1]:
        lines[i] = line.replace('name: Ident {', 'path: vec![Ident {')
        lines[i+3] = lines[i+3].replace('},', '}],')

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.writelines(lines)
