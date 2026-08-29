import re
with open('crates/kai-typecheck/src/expr/mod.rs', 'r') as f:
    text = f.read()

old_ident = """fn ident_ref(checker: &mut Checker, ident: &Ident) -> TypedExpr {
    if let Some(info) = checker.locals.lookup(&ident.name) {
        return TypedExpr::new(TypedExprKind::LocalRef(info.id), info.ty.clone());
    }
    
    // Fallback: check if it's a local type
    if let Some(&idx) = checker.local_types().get(&ident.name) {
        return TypedExpr::new(TypedExprKind::TypeRef(idx), KaiType::NAMESPACE);
    }
    
    // Fallback: check if it's a module alias
    let m_idx = checker.current_module;
    if let Some(&target) = checker.resolution.imports[m_idx].get(&ident.name) {
        return TypedExpr::new(TypedExprKind::ModuleRef(target), KaiType::NAMESPACE);
    }
    
    let span = ident.span;
    let name = ident.name.clone();
    checker.error(error::undeclared_variable(&name, span));
    zero_int()
}"""

new_ident = """fn ident_ref(checker: &mut Checker, ident: &Ident) -> TypedExpr {
    if let Some(info) = checker.locals.lookup(&ident.name) {
        return TypedExpr::new(TypedExprKind::LocalRef(info.id), info.ty.clone());
    }
    
    if let Some(&idx) = checker.local_fns().get(&ident.name) {
        return TypedExpr::new(TypedExprKind::FnRef(kai_tast::symbol::FunctionId(idx as u32)), KaiType::NAMESPACE);
    }
    
    if let Some(&idx) = checker.local_types().get(&ident.name) {
        return TypedExpr::new(TypedExprKind::TypeRef(idx), KaiType::NAMESPACE);
    }
    
    let m_idx = checker.current_module;
    if let Some(&target) = checker.resolution.imports[m_idx].get(&ident.name) {
        return TypedExpr::new(TypedExprKind::ModuleRef(target), KaiType::NAMESPACE);
    }
    
    let span = ident.span;
    let name = ident.name.clone();
    checker.error(error::undeclared_variable(&name, span));
    zero_int()
}"""

text = text.replace(old_ident, new_ident)
with open('crates/kai-typecheck/src/expr/mod.rs', 'w') as f:
    f.write(text)
