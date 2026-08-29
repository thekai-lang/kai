with open('crates/kai-typecheck/src/decl.rs', 'r') as f:
    lines = f.readlines()

for i in [93, 158]: # 0-indexed for lines 94 and 159
    lines[i] = lines[i].replace('decl.name.name.clone()', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')

for i in [218]: # line 219
    lines[i] = lines[i].replace('decl.name.name.clone()', 'decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".")')

with open('crates/kai-typecheck/src/decl.rs', 'w') as f:
    f.writelines(lines)
