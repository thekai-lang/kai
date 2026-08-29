with open('crates/kai-tast/src/expr.rs', 'r') as f:
    text = f.read()

# Add ModuleRef, TypeRef, FnRef at the end of TypedExprKind
variants = """
    UnwrapOr { receiver: Box<TypedExpr>, default: Box<TypedExpr> },
    ModuleRef(usize),
    TypeRef(usize),
    FnRef(crate::symbol::FunctionId),
}"""
text = text.replace('UnwrapOr { receiver: Box<TypedExpr>, default: Box<TypedExpr> },\n}', variants)

with open('crates/kai-tast/src/expr.rs', 'w') as f:
    f.write(text)

with open('crates/kai-tast/src/ty.rs', 'r') as f:
    text = f.read()

# Add Namespace to KaiType
variants = """
    Error,
    Namespace,
}"""
text = text.replace('    Error,\n}', variants)

# Add Display logic for Namespace
display = """            KaiType::Error => write!(f, "<error>"),
            KaiType::Namespace => write!(f, "<namespace>"),"""
text = text.replace('KaiType::Error => write!(f, "<error>"),', display)

with open('crates/kai-tast/src/ty.rs', 'w') as f:
    f.write(text)
