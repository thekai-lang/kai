import re
with open('crates/kai-ownership/src/walk.rs', 'r') as f:
    text = f.read()

text = text.replace('hoist_borrow_temps(heap, e, fresh, scopes, &mut out, true);', 'hoist_borrow_temps(heap, e, fresh, scopes, &mut out, true, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut binding.init, fresh, scopes, &mut out, true);', 'hoist_borrow_temps(heap, &mut binding.init, fresh, scopes, &mut out, true, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, idx, fresh, scopes, &mut out, false);', 'hoist_borrow_temps(heap, idx, fresh, scopes, &mut out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut assign.value, fresh, scopes, &mut out, true);', 'hoist_borrow_temps(heap, &mut assign.value, fresh, scopes, &mut out, true, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut if_.cond, fresh, scopes, &mut out, false);', 'hoist_borrow_temps(heap, &mut if_.cond, fresh, scopes, &mut out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut while_.cond, fresh, scopes, &mut pre, false);', 'hoist_borrow_temps(heap, &mut while_.cond, fresh, scopes, &mut pre, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);', 'hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false, is_reversible);')
text = text.replace('hoist_borrow_temps(heap, e, fresh, scopes, &mut done, true);', 'hoist_borrow_temps(heap, e, fresh, scopes, &mut done, true, is_reversible);')

with open('crates/kai-ownership/src/walk.rs', 'w') as f:
    f.write(text)
