import re
with open('crates/kai-typecheck/src/expr/call.rs', 'r') as f:
    text = f.read()

text = re.sub(r'pub\(crate\) fn qualified_callee.*?Some\(FunctionId\(idx as u32\)\)\n}\n', '', text, flags=re.DOTALL)

with open('crates/kai-typecheck/src/expr/call.rs', 'w') as f:
    f.write(text)
