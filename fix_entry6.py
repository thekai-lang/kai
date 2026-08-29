import re
with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = text.replace('decl.name.name', 'decl.path.last().unwrap().name')
text = text.replace('f.name.span', 'f.path.last().unwrap().span')
text = text.replace('main.name.span', 'main.path.last().unwrap().span')
text = text.replace('Ty::Named(ident) if ident.name == "int32" || ident.name == "int"', 'Ty::Path(path) if path.len() == 1 && (path[0].name == "int32" || path[0].name == "int")')

text = re.sub(
    r'Ty::Named\(Ident \{\s*name: name\.into\(\),\s*span: Span::new\(0, 0\),\s*\}\)',
    r'Ty::Path(vec![Ident { name: name.into(), span: Span::new(0, 0) }])',
    text
)

text = re.sub(
    r'name: Ident \{\s*name: name\.into\(\),\s*span: Span::new\(0, 0\),\s*\},',
    r'path: vec![Ident { name: name.into(), span: Span::new(0, 0) }],',
    text
)

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)
