import re
with open('crates/kai-typecheck/src/expr/struct_lit.rs', 'r') as f:
    text = f.read()

old_fa = """pub(crate) fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = lower(checker, &access.base, None);
    let Some((struct_id, field, ty)) =
        resolve_field_hop(checker, &base.ty, &access.field.name, access.field.span)
    else {
        return poisoned();
    };
    TypedExpr::new(
        TypedExprKind::FieldAccess {
            base: Box::new(base),
            struct_id,
            field,
        },
        ty,
    )
}"""

new_fa = """pub(crate) fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = lower(checker, &access.base, None);
    
    // Module namespace lookup
    if let TypedExprKind::ModuleRef(target_mod) = base.kind {
        let name = &access.field.name;
        if let Some(&idx) = checker.resolution.module_types[target_mod].get(name) {
            if checker.resolution.type_is_public[idx] {
                return TypedExpr::new(TypedExprKind::TypeRef(idx), KaiType::Namespace);
            } else {
                checker.error(kai_diagnostics::Diagnostic::error(format!("type `{name}` is private"), access.field.span).with_file(&checker.cur_file));
                return poisoned();
            }
        }
        // Direct symbol import (or just module function)
        if let Some(&idx) = checker.resolution.module_fns[target_mod].get(name) {
            if checker.resolution.fn_is_public[idx] {
                // Return a function ref! Wait, TAST has no FnRef expr, it only has Call!
                // But in Kai, functions are not first-class values (yet).
                // If it's a function, it MUST be inside a Call expression.
                // Wait, if it MUST be inside a call, `FieldAccess` cannot resolve to a function!
                // We'll see how `Call` is parsed.
            }
        }
        checker.error(kai_diagnostics::Diagnostic::error(format!("module has no public member `{name}`"), access.field.span).with_file(&checker.cur_file));
        return poisoned();
    }
    
    // Type namespace lookup (for associated functions)
    if let TypedExprKind::TypeRef(type_idx) = base.kind {
        // Associated function lookup.
        // Wait, same issue: we don't have a `FnRef` variant.
        // Let's add a dummy variant just so `Call` can unwrap it, OR we intercept it in `call.rs`.
    }

    let Some((struct_id, field, ty)) =
        resolve_field_hop(checker, &base.ty, &access.field.name, access.field.span)
    else {
        return poisoned();
    };
    TypedExpr::new(
        TypedExprKind::FieldAccess {
            base: Box::new(base),
            struct_id,
            field,
        },
        ty,
    )
}"""

# Actually, if we intercept it in `call.rs`, we don't need FnRef.
# BUT wait! `user.User.change_email(...)` is parsed as `Call(FieldAccess(user.User, change_email))`.
# So `FieldAccess` WILL execute for `change_email`!
# If `FieldAccess` executes for `change_email`, it MUST return something so `Call` can use it!
# Let's add `FnRef(FunctionId)` to TAST `TypedExprKind`!
