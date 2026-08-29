with open('crates/kai-parser/src/ty.rs', 'r') as f:
    text = f.read()

text = text.replace('''            Ty::Named(Ident {
                name: String::new(),
                span: found.span,
            })''', '            Ty::Path(vec![Ident { name: String::new(), span: found.span }])')

text = text.replace('''fn named_ty(parser: &mut Parser, head: String) -> Ty {
    let tok = parser.bump();
    let ident = Ident {
        name: head,
        span: tok.span,
    };
    if !parser.eat_simple(&TokenKind::Lt) {
        return Ty::Named(ident);
    }
    match ident.name.as_str() {''', '''fn named_ty(parser: &mut Parser, head: String) -> Ty {
    let tok = parser.bump();
    let mut path = vec![Ident {
        name: head,
        span: tok.span,
    }];
    
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a type name segment"));
    }

    if !parser.eat_simple(&TokenKind::Lt) {
        return Ty::Path(path);
    }
    match path.last().unwrap().name.as_str() {''')

text = text.replace('''                    err: Box::new(Ty::Named(Ident {
                        name: String::new(),
                        span: found.span,
                    })),''', '''                    err: Box::new(Ty::Path(vec![Ident {
                        name: String::new(),
                        span: found.span,
                    }])),''')

text = text.replace('''            skip_type_args(parser);
            Ty::Named(ident)''', '''            skip_type_args(parser);
            Ty::Path(path)''')
            
text = text.replace('ident.span', 'path.last().unwrap().span')

with open('crates/kai-parser/src/ty.rs', 'w') as f:
    f.write(text)
