import re
with open('crates/kai-ast/src/use_decl.rs', 'r') as f:
    text = f.read()

text = text.replace(
    'pub path: Vec<Ident>,',
    'pub path: Vec<Ident>,\n    pub as_alias: Option<Ident>,'
)

text = text.replace(
    'pub fn alias(&self) -> Option<&Ident> {\n        self.path.last()\n    }',
    'pub fn alias(&self) -> Option<&Ident> {\n        self.as_alias.as_ref().or_else(|| self.path.last())\n    }'
)

with open('crates/kai-ast/src/use_decl.rs', 'w') as f:
    f.write(text)
