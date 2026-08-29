import re
with open('crates/kai-parser/src/decl.rs', 'r') as f:
    text = f.read()

old = """fn fn_decl(parser: &mut Parser) -> FnDecl {
    let start = parser.span_here();
    let is_public = parser.eat_simple(&TokenKind::Public);
    parser.bump(); // `fn`

    let name = parser.expect_ident("a function name");"""

new = """fn fn_decl(parser: &mut Parser) -> FnDecl {
    let start = parser.span_here();
    let is_public = parser.eat_simple(&TokenKind::Public);
    parser.bump(); // `fn`

    let mut path = vec![parser.expect_ident("a function name")];
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a function name segment"));
    }"""

text = text.replace(old, new)
text = text.replace('name,\n        params,', 'path,\n        params,')

with open('crates/kai-parser/src/decl.rs', 'w') as f:
    f.write(text)
