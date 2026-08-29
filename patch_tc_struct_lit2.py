import re
with open('crates/kai-typecheck/src/expr/struct_lit.rs', 'r') as f:
    text = f.read()

new_fa = """pub(crate) fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = lower(checker, &access.base, None);
    
    // Module namespace lookup
    if let TypedExprKind::ModuleRef(target_mod) = base.kind {
        let name = &access.field.name;
        if let Some(&idx) = checker.resolution.module_types[target_mod].get(name) {
            if checker.resolution.type_is_public[idx] {
                return TypedExpr::new(TypedExprKind::TypeRef(idx), KaiType::Namespace);
            } else {
                checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("type `{name}` is private"), access.field.span).with_file(&checker.cur_file));
                return poisoned();
            }
        }
        if let Some(&idx) = checker.resolution.module_fns[target_mod].get(name) {
            if checker.resolution.fn_is_public[idx] {
                return TypedExpr::new(TypedExprKind::FnRef(crate::symbol::FunctionId(idx as u32)), KaiType::Namespace);
            } else {
                checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("function `{name}` is private"), access.field.span).with_file(&checker.cur_file));
                return poisoned();
            }
        }
        checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("module has no public member `{name}`"), access.field.span).with_file(&checker.cur_file));
        return poisoned();
    }
    
    // Type namespace lookup (for associated functions)
    if let TypedExprKind::TypeRef(type_idx) = base.kind {
        // Associated function lookup.
        let type_name = checker.type_name(StructId(type_idx as u32));
        let type_name = type_name.to_string();
        let name = &access.field.name;
        let assoc_name = format!("{type_name}.{name}");
        let owner_mod = checker.resolution.type_module[type_idx];
        
        if let Some(&idx) = checker.resolution.module_fns[owner_mod].get(&assoc_name) {
            let is_same_mod = checker.current_module == owner_mod;
            if checker.resolution.fn_is_public[idx] || is_same_mod {
                return TypedExpr::new(TypedExprKind::FnRef(crate::symbol::FunctionId(idx as u32)), KaiType::Namespace);
            } else {
                checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("associated function `{assoc_name}` is private"), access.field.span).with_file(&checker.cur_file));
                return poisoned();
            }
        }
        
        checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("type `{type_name}` has no associated function `{name}`"), access.field.span).with_file(&checker.cur_file));
        return poisoned();
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

# we already ran patch_tc_struct_lit.py but it failed to replace because I didn't actually run replace in it!
# wait, my previous script had `python3 patch_tc_struct_lit.py` but I didn't actually do `text = text.replace(old, new)` inside it.
# So I'll do it now!
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
text = text.replace(old_fa, new_fa)

with open('crates/kai-typecheck/src/expr/struct_lit.rs', 'w') as f:
    f.write(text)
