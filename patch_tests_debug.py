import re
with open('crates/kai-typecheck/src/v0012_tests.rs', 'r') as f:
    text = f.read()

text = text.replace('assert!(err.contains("type `Secret` is private") || err.contains("has no type `Secret`"));', 'assert!(err.contains("type `Secret` is private") || err.contains("has no type `Secret`"), "{}", err);')

with open('crates/kai-typecheck/src/v0012_tests.rs', 'w') as f:
    f.write(text)
