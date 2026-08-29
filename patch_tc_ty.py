import re
with open('crates/kai-typecheck/src/ty.rs', 'r') as f:
    text = f.read()

# 12 |         Ty::Named(ident) => match ident.name.as_str() {
text = re.sub(
    r'Ty::Named\((ident.*?)\)\s*=>\s*match\s*\1\.name\.as_str\(\)\s*\{',
    r'Ty::Path(path) => match if path.len() == 1 { path[0].name.as_str() } else { "" } {',
    text
)
# What if it's not a primitive?
# Then it drops to the default case in that match:
# _ => {
#    let global_id = ...
# }
