with open('crates/kai-typecheck/src/expr/mod.rs', 'r') as f:
    text = f.read()

text = text.replace('pub(crate) fn lower(checker: &mut Checker, expr: &Expr, expected: Option<KaiType>) -> TypedExpr {', 'pub(crate) fn lower_namespace_aware(checker: &mut Checker, expr: &Expr, expected: Option<KaiType>) -> TypedExpr {')

wrapper = """
pub(crate) fn lower(checker: &mut Checker, expr: &Expr, expected: Option<KaiType>) -> TypedExpr {
    let mut typed = lower_namespace_aware(checker, expr, expected);
    match typed.kind {
        TypedExprKind::ModuleRef(_) | TypedExprKind::TypeRef(_) | TypedExprKind::FnRef(_) => {
            checker.diagnostics.push(kai_diagnostics::Diagnostic::error("symbol cannot be used as a value".to_string(), expr.span).with_file(&checker.cur_file));
            typed.kind = TypedExprKind::Invalid;
        }
        _ => {}
    }
    typed
}
"""
text = text + wrapper

with open('crates/kai-typecheck/src/expr/mod.rs', 'w') as f:
    f.write(text)

with open('crates/kai-typecheck/src/expr/struct_lit.rs', 'r') as f:
    text = f.read()
text = text.replace('let base = lower(checker, &access.base, None);', 'let base = super::lower_namespace_aware(checker, &access.base, None);')
with open('crates/kai-typecheck/src/expr/struct_lit.rs', 'w') as f:
    f.write(text)

with open('crates/kai-typecheck/src/expr/call.rs', 'r') as f:
    text = f.read()
text = text.replace('let callee_val = lower(checker, &call.callee, None);', 'let callee_val = super::lower_namespace_aware(checker, &call.callee, None);')
with open('crates/kai-typecheck/src/expr/call.rs', 'w') as f:
    f.write(text)
