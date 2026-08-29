with open('crates/kai-tast/src/expr.rs', 'r') as f:
    text = f.read()

new_variants = """
    UnwrapOr { receiver: Box<TypedExpr>, default: Box<TypedExpr> },
    // v0.0.12 - Compile-time semantic resolution nodes.
    // These never reach codegen. If they do, they panic.
    ModuleRef(usize),
    TypeRef(usize),
}"""

text = text.replace('UnwrapOr { receiver: Box<TypedExpr>, default: Box<TypedExpr> },\n}', new_variants)

with open('crates/kai-tast/src/expr.rs', 'w') as f:
    f.write(text)

with open('crates/kai-tast/src/ty.rs', 'r') as f:
    text = f.read()
# Let's add Namespace type to KaiType
text = text.replace('Error,\n}', 'Error,\n    Namespace,\n}')
with open('crates/kai-tast/src/ty.rs', 'w') as f:
    f.write(text)

