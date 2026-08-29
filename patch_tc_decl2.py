import re
with open('crates/kai-typecheck/src/decl.rs', 'r') as f:
    text = f.read()

# Lines 94, 159, 219 are for FnDecl
idx1 = text.find('let name = decl.name.name.clone();') # inside fn check_fn
text = text.replace('let name = decl.name.name.clone();', 'let name = decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");')

idx2 = text.find('name: decl.name.name.clone(),') # maybe inside extract_signature
# wait, TypeDecl might also have this.
