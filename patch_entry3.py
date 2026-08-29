with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = text.replace('decl.name.name', 'decl.path.last().unwrap().name')
text = text.replace('f.name.span', 'f.path.last().unwrap().span')
text = text.replace('main.name.span', 'main.path.last().unwrap().span')

text = text.replace('Ty::Named(ident) if ident.name == "int32" || ident.name == "int"', 'Ty::Path(path) if path.len() == 1 && (path[0].name == "int32" || path[0].name == "int")')

text = text.replace('''        Ty::Named(Ident {
            name: "int32".to_string(),
            span: Span::new(0, 0),
        })''', '''        Ty::Path(vec![Ident {
            name: "int32".to_string(),
            span: Span::new(0, 0),
        }])''')

text = text.replace('''            name: Ident {
                name: "main".to_string(),
                span: Span::new(0, 0),
            },''', '''            path: vec![Ident {
                name: "main".to_string(),
                span: Span::new(0, 0),
            }],''')

text = text.replace('''        main.params.push(Param {
            name: Ident {
                name: "args".to_string(),
                span: Span::new(0, 0),
            },
            ty: Ty::Named(Ident {
                name: "string".to_string(),
                span: Span::new(0, 0),
            }),
            span: Span::new(0, 0),
        });''', '''        main.params.push(Param {
            name: Ident {
                name: "args".to_string(),
                span: Span::new(0, 0),
            },
            ty: Ty::Path(vec![Ident {
                name: "string".to_string(),
                span: Span::new(0, 0),
            }]),
            span: Span::new(0, 0),
        });''')

with open('crates/kai-resolver/src/entry.rs', 'w') as f:
    f.write(text)
