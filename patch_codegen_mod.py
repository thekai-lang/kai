with open('crates/kai-codegen/src/emit/expr/mod.rs', 'r') as f:
    text = f.read()

text = text.replace(
    'TypedExprKind::Invalid => undef_of(ctx, &ty),',
    'TypedExprKind::Invalid => undef_of(ctx, &ty),\n        TypedExprKind::ModuleRef(_) | TypedExprKind::TypeRef(_) | TypedExprKind::FnRef(_) => unreachable!("Compile-time semantic node reached codegen"),'
)

with open('crates/kai-codegen/src/emit/expr/mod.rs', 'w') as f:
    f.write(text)
