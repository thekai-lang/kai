with open('crates/kai-lexer/src/keywords.rs', 'r') as f:
    text = f.read()
text = text.replace(
    '"public" => TokenKind::Public,',
    '"public" => TokenKind::Public,\n        "as" => TokenKind::As,'
)
with open('crates/kai-lexer/src/keywords.rs', 'w') as f:
    f.write(text)
