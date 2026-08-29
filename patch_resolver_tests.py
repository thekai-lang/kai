with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = text.replace('Ty::Named(Ident {', 'Ty::Path(vec![Ident {')
text = text.replace('span: Span::new(0, 0),\n                })', 'span: Span::new(0, 0),\n                }])')
text = text.replace('name: Ident {', 'path: vec![Ident {')
text = text.replace('span: Span::new(0, 0),\n            },', 'span: Span::new(0, 0),\n            }],')

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)
