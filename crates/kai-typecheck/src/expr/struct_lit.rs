#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
use super::*;

pub(crate) fn field_access(checker: &mut Checker, access: &FieldAccessExpr) -> TypedExpr {
    let base = super::lower_namespace_aware(checker, &access.base, None);
    
    // Module namespace lookup
    if let TypedExprKind::ModuleRef(target_mod) = base.kind {
        let name = &access.field.name;
        if let Some(&idx) = checker.resolution.module_types[target_mod].get(name) {
            if checker.resolution.type_is_public[idx] {
                return TypedExpr::new(TypedExprKind::TypeRef(idx), KaiType::NAMESPACE);
            } else {
                checker.diagnostics.push(kai_diagnostics::Diagnostic::error(format!("type `{name}` is private"), access.field.span).with_file(&checker.cur_file));
                return poisoned();
            }
        }
        if let Some(&idx) = checker.resolution.module_fns[target_mod].get(name) {
            if checker.resolution.fn_is_public[idx] {
                return TypedExpr::new(TypedExprKind::FnRef(kai_tast::symbol::FunctionId(idx as u32)), KaiType::NAMESPACE);
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
                return TypedExpr::new(TypedExprKind::FnRef(kai_tast::symbol::FunctionId(idx as u32)), KaiType::NAMESPACE);
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
}

/// `Name { f: e, .. }` — every field exactly once, in any source order; the
/// lowered values are reordered into declaration order (the ABI layout).
/// The head is either an unqualified name (own module) or
/// `alias.Name` naming a PUBLIC struct of an imported module.

pub(crate) fn struct_lit(checker: &mut Checker, lit: &StructLitExpr, lit_span: Span) -> TypedExpr {
    let segments = lit.path.len();
    let struct_id = if segments == 1 {
        // Unqualified: own module only.
        let type_name = &lit.path[0];
        match checker.local_types().get(&type_name.name) {
            Some(&idx) => StructId(idx as u32),
            None => {
                checker.error(error::unknown_type(&type_name.name, type_name.span));
                return poisoned();
            }
        }
    } else {
        // Qualified head: first segment must be an import alias.
        let alias = &lit.path[0];
        let member = lit.path.last().expect("non-empty literal head");
        let path = format!(
            "{}.{}",
            alias.name,
            lit.path[1..]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".")
        );
        match checker.imports().get(&alias.name) {
            Some(&target) => {
                let Some(&idx) = checker.resolution.module_types[target].get(&member.name)
                else {
                    checker.error(error::unknown_qualified_type(&path, member.span));
                    return poisoned();
                };
                if !checker.resolution.type_is_public[idx] {
                    checker.error(error::private_type(&path, member.span));
                    return poisoned();
                }
                StructId(idx as u32)
            }
            None => {
                checker.error(error::unknown_module(&alias.name, alias.span));
                return poisoned();
            }
        }
    };
    let ty_name = checker.type_name(struct_id).to_string();
    let layout_len = checker.structs[struct_id.0 as usize].fields.len();

    // provided[i] = Some(value) once field i has been initialized.
    let mut provided: Vec<Option<TypedExpr>> = vec![None; layout_len];
    let mut seen_dup: Vec<bool> = vec![false; layout_len];

    for init in &lit.fields {
        match checker.field_slot(struct_id, &init.name.name) {
            Some((index, slot)) => {
                let expected_ty = slot.ty.clone();
                if seen_dup[index as usize] {
                    let field = init.name.name.clone();
                    checker.error(error::duplicate_field_init(&field, init.name.span));
                } else {
                    seen_dup[index as usize] = true;
                    let value = lower(checker, &init.value, Some(expected_ty.clone()));
                    // The hint widens int literals; everything else must
                    // match the declared field type exactly.
                    if value.ty != expected_ty {
                        let field = init.name.name.clone();
                        checker.error(error::field_type_mismatch(
                            &field,
                            expected_ty.clone(),
                            value.ty.clone(),
                            init.value.span,
                        ));
                    }
                    provided[index as usize] = Some(value);
                }
            }
            None => {
                let field = init.name.name.clone();
                checker.error(error::no_such_field(&ty_name, &field, init.name.span));
                // Lower anyway so nested errors surface too.
                lower(checker, &init.value, None);
            }
        }
    }

    let mut values = Vec::with_capacity(layout_len);
    for (slot_index, value) in provided.into_iter().enumerate() {
        match value {
            Some(v) => values.push(v),
            None => {
                let field = checker.structs[struct_id.0 as usize].fields[slot_index]
                    .name
                    .clone();
                checker.error(error::missing_field_in_lit(&field, &ty_name, lit_span));
                values.push(poisoned());
            }
        }
    }

    TypedExpr::new(
        TypedExprKind::StructLit { struct_id, values },
        KaiType::Struct(struct_id),
    )
}

// -- v0.0.5: strings, arrays, indexing ---------------------------------------

/// `[e0, e1, ..]`: every element unifies to ONE type; the context hint (an
/// expected `T[]`) types bare int literals and — decisively — makes an
/// EMPTY literal legal. `let a = [];` with no annotation is an error
/// (§9.7): there is nothing to infer from.

pub(crate) fn resolve_field_hop(
    checker: &mut Checker,
    cur: &KaiType,
    field: &str,
    span: Span,
) -> Option<(StructId, u16, KaiType)> {
    let struct_id = match cur {
        KaiType::Struct(id) => *id,
        other => {
            checker.error(error::field_access_on_non_struct(other.clone(), span));
            return None;
        }
    };
    match checker.field_slot(struct_id, field) {
        Some((index, slot)) => Some((struct_id, index, slot.ty.clone())),
        None => {
            let ty_name = checker.type_name(struct_id).to_string();
            checker.error(error::no_such_field(&ty_name, field, span));
            None
        }
    }
}

/// One `[..]` hop: array-shape check plus a lowered, integer-checked index
/// expression (§9.3).

pub(crate) fn resolve_index_hop(
    checker: &mut Checker,
    cur: &KaiType,
    index: &Expr,
    rbracket: Span,
) -> Option<(KaiType, TypedExpr)> {
    let elem_ty = match cur {
        KaiType::Array(elem) => elem.as_ref().clone(),
        other => {
            checker.error(error::index_on_non_array(other, rbracket));
            return None;
        }
    };
    let typed_index = lower(checker, index, None);
    if !typed_index.ty.is_integer() {
        let ty = typed_index.ty.clone();
        checker.error(error::index_not_integer(&ty, rbracket));
    }
    Some((elem_ty, typed_index))
}
