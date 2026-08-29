import re
with open('crates/kai-typecheck/src/v0012_tests.rs', 'r') as f:
    text = f.read()

text = text.replace('assert!(err.contains("must be defined in the same module that owns the type"));', 'assert!(err.contains("must be defined in the same module that owns the type"), "{}", err);')

with open('crates/kai-typecheck/src/v0012_tests.rs', 'w') as f:
    f.write(text)
