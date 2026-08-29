import re
with open('crates/kai-ast/src/fn_decl.rs', 'r') as f:
    text = f.read()

text = text.replace(
    'pub name: Ident,',
    'pub path: Vec<Ident>,'
)

with open('crates/kai-ast/src/fn_decl.rs', 'w') as f:
    f.write(text)
