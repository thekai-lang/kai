import re
with open('crates/kai-parser/src/decl.rs', 'r') as f:
    text = f.read()

old = """fn use_decl(parser: &mut Parser) -> UseDecl {
    let start = parser.span_here();
    parser.bump(); // `use`

    let mut path = vec![parser.expect_ident("a module path segment")];
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a module path segment"));
    }
    let end = parser.expect_simple(&TokenKind::Semi);
    let span = Span::merge(start, end);
    UseDecl { path, span }
}"""

new = """fn use_decl(parser: &mut Parser) -> UseDecl {
    let start = parser.span_here();
    parser.bump(); // `use`

    let mut path = vec![parser.expect_ident("a module path segment")];
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a module path segment"));
    }
    
    let mut as_alias = None;
    if parser.eat_simple(&TokenKind::As) {
        as_alias = Some(parser.expect_ident("an alias name"));
    }
    
    let end = parser.expect_simple(&TokenKind::Semi);
    let span = Span::merge(start, end);
    UseDecl { path, as_alias, span }
}"""

text = text.replace(old, new)
with open('crates/kai-parser/src/decl.rs', 'w') as f:
    f.write(text)
