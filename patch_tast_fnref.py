with open('crates/kai-tast/src/expr.rs', 'r') as f:
    text = f.read()

text = text.replace('TypeRef(usize),\n}', 'TypeRef(usize),\n    FnRef(crate::symbol::FunctionId),\n}')
with open('crates/kai-tast/src/expr.rs', 'w') as f:
    f.write(text)
