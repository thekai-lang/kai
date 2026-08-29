import re
with open('crates/kai-ownership/src/hoist.rs', 'r') as f:
    text = f.read()

text = text.replace('hoist_children(heap, rhs, fresh, &mut throwaway, &mut conditional_out);', 'hoist_children(heap, rhs, fresh, &mut throwaway, &mut conditional_out, is_reversible);')
text = text.replace('hoist_root(heap, rhs, fresh, &mut throwaway, &mut conditional_out, false, false);', 'hoist_root(heap, rhs, fresh, &mut throwaway, &mut conditional_out, false, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, value, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, value, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, receiver, fresh, scopes, out, false);', 'hoist_borrow_temps(heap, receiver, fresh, scopes, out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, base, fresh, scopes, out, false)', 'hoist_borrow_temps(heap, base, fresh, scopes, out, false, is_reversible)')
text = text.replace('let init =', 'let mut init =')

with open('crates/kai-ownership/src/hoist.rs', 'w') as f:
    f.write(text)
