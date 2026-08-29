import re
with open('crates/kai-typecheck/src/expr/call.rs', 'r') as f:
    text = f.read()

# We'll replace the block from `let func_id = match &call.callee.kind {` up to `};`
old_call = """    let func_id = match &call.callee.kind {
        ExprKind::Ident(ident) => match checker.local_fns().get(&ident.name) {
            Some(&idx) => FunctionId(idx as u32),
            None => {
                // Not a declared function — maybe a closure-VALUED local
                // (v0.0.6 first-class calls).
                if checker
                    .locals
                    .lookup(&ident.name)
                    .is_some_and(|info| matches!(info.ty, KaiType::Closure { .. }))
                    && let Some(t) = try_closure_call(checker, call, span)
                {
                    return t;
                }
                checker.error(error::unknown_function(&ident.name, ident.span));
                return poisoned();
            }
        },
        ExprKind::FieldAccess(access) => {
            if !is_import_alias(checker, &access.base)
                && access.field.name != "unwrap_or"
            {
                // Maybe a closure stored in a field / reached by projection.
                if let Some(t) = try_closure_call(checker, call, span) {
                    return t;
                }
            }
            match qualified_callee(checker, access) {
                Some(id) => id,
                None => return poisoned(),
            }
        }
        _ => {
            // Not a named function: maybe a closure VALUE. Typing supports
            // the call; emission goes indirect (P5).
            if let Some(t) = try_closure_call(checker, call, span) {
                return t;
            }
            checker.error(error::indirect_call(span));
            return poisoned();
        }
    };"""

new_call = """    let func_id = {
        let callee_val = lower(checker, &call.callee, None);
        if let TypedExprKind::FnRef(id) = callee_val.kind {
            id
        } else {
            // Fallback for closures or local fns not caught by FnRef?
            // Actually, ident_ref currently doesn't check local_fns! It only checks locals.
            // Let's modify ident_ref to return FnRef if it's a local function!
            // Wait, what if we just check local_fns here? No, if it's a local function, `lower` will fail with "undeclared variable".
            // So we MUST change ident_ref to check local_fns!
            
            // For now, let's keep the old behavior for Ident and closure:
            match &call.callee.kind {
                ExprKind::Ident(ident) => {
                    if let Some(&idx) = checker.local_fns().get(&ident.name) {
                        FunctionId(idx as u32)
                    } else if let Some(t) = try_closure_call(checker, call, span) {
                        return t;
                    } else {
                        checker.error(error::unknown_function(&ident.name, ident.span));
                        return poisoned();
                    }
                },
                _ => {
                    if let Some(t) = try_closure_call(checker, call, span) {
                        return t;
                    }
                    checker.error(error::indirect_call(span));
                    return poisoned();
                }
            }
        }
    };"""

text = text.replace(old_call, new_call)
with open('crates/kai-typecheck/src/expr/call.rs', 'w') as f:
    f.write(text)
