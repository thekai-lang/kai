import re
with open('docs/kai-ebnf.md', 'r') as f:
    text = f.read()

text = text.replace('''FnDecl ::= "public"? "fn" Ident "(" ParamList ")" "->" Type [ "reversible" ] Block''', '''FnDecl ::= "public"? "fn" Path "(" ParamList ")" "->" Type [ "reversible" ] Block''')
text = text.replace('''Type ::= BaseType { '[' ']' } [ '?' ]
BaseType ::= Ident [ '<' Type [ ',' Type ] '>' ]''', '''Type ::= BaseType { '[' ']' } [ '?' ]
BaseType ::= Path [ '<' Type [ ',' Type ] '>' ]
Path ::= Ident { '.' Ident }''')

text = text.replace('''UseDecl ::= "use" Path [ "as" Ident ] ";"
Path ::= Ident { "." Ident }''', '''UseDecl ::= "use" Path [ "as" Ident ] ";"''')

if '## 12. Module Behavior' not in text:
    text += '''\n## 12. Module Behavior & Associated Types — v0.0.12

```ebnf
FnDecl ::= "public"? "fn" Path "(" ParamList ")" "->" Type [ "reversible" ] Block
Type ::= Path { '[' ']' } [ '?' ]
UseDecl ::= "use" Path [ "as" Ident ] ";"
```
*Note: `Path` replacing `Ident` in `FnDecl` allows for associated functions (`fn User.create()`). In `Type`, it allows for module-qualified types (`auth.User`). In `UseDecl`, `Path` can now resolve to a specific symbol for direct import.*
'''

with open('docs/kai-ebnf.md', 'w') as f:
    f.write(text)
