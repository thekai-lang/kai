import re
with open('crates/kai-ownership/src/hoist.rs', 'r') as f:
    text = f.read()

text = text.replace('pub(crate) fn hoist_borrow_temps(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n    root_is_transfer: bool,\n) {',
'pub(crate) fn hoist_borrow_temps(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n    root_is_transfer: bool,\n    is_reversible: bool,\n) {')

text = text.replace('hoist_children(heap, expr, fresh, scopes, out);', 'hoist_children(heap, expr, fresh, scopes, out, is_reversible);')
text = text.replace('hoist_root(heap, expr, fresh, scopes, out, root_is_transfer, true);', 'hoist_root(heap, expr, fresh, scopes, out, root_is_transfer, true, is_reversible);')

text = text.replace('fn hoist_children(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n) {',
'fn hoist_children(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n    is_reversible: bool,\n) {')

text = text.replace('hoist_borrow_temps(heap, base, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, base, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, lhs, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, lhs, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, rhs, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, rhs, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, a, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, a, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, index, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, index, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, e, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, e, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, v, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, v, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, r_expr, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, r_expr, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut f.expr, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, &mut f.expr, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, inner, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, inner, fresh, scopes, out, false, is_reversible);')

text = text.replace('fn hoist_root(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n    root_is_transfer: bool,\n    register_scope: bool,\n) {',
'fn hoist_root(\n    heap: &HeapBearing,\n    expr: &mut TypedExpr,\n    fresh: &mut FreshIds,\n    scopes: &mut Scopes,\n    out: &mut Vec<TypedStmt>,\n    root_is_transfer: bool,\n    register_scope: bool,\n    is_reversible: bool,\n) {')

text = text.replace('''        if register_scope {
            scopes.declare(local, ty, true);
        }
        out.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$tmp".into(),
            init,
        }));''',
'''        if register_scope {
            scopes.declare(local, ty, true);
        }
        crate::walk::walk_expr(heap, &mut init, scopes, fresh, is_reversible);
        crate::heap::wrap_retain_if_borrowed(heap, &mut init);
        out.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$tmp".into(),
            init,
        }));''')

with open('crates/kai-ownership/src/hoist.rs', 'w') as f:
    f.write(text)
