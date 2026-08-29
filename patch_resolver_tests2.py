with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

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
