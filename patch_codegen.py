import re
with open('crates/kai-codegen/src/emit/expr/mod.rs', 'r') as f:
    text = f.read()

text = text.replace(
    'TypedExprKind::Invalid => ctx.lower_invalid(),\n    }',
    'TypedExprKind::Invalid => ctx.lower_invalid(),\n        TypedExprKind::ModuleRef(_) | TypedExprKind::TypeRef(_) | TypedExprKind::FnRef(_) => unreachable!("Compile-time semantic node reached codegen"),\n    }'
)

with open('crates/kai-codegen/src/emit/expr/mod.rs', 'w') as f:
    f.write(text)
