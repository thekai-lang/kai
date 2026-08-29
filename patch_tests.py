import re
with open('crates/kai-typecheck/src/v0012_tests.rs', 'r') as f:
    text = f.read()

text = text.replace('fn test(u: e.User) -> e.User { return e.User.create(); }', 'fn main() -> int32 { return 0; } fn test(u: e.User) -> e.User { return e.User.create(); }')
text = text.replace('fn test() -> User { return User.create(); }', 'fn main() -> int32 { return 0; } fn test() -> User { return User.create(); }')
text = text.replace('fn test() -> e.Secret { return 0; }', 'fn main() -> int32 { return 0; } fn test() -> e.Secret { return 0; }')
text = text.replace('fn test() -> unit { let x = entity.User; }', 'fn main() -> int32 { return 0; } fn test() -> unit { let x = entity.User; }')
text = text.replace('fn User.hack() -> unit {}\\n', 'fn main() -> int32 { return 0; } fn User.hack() -> unit {}\\n')

with open('crates/kai-typecheck/src/v0012_tests.rs', 'w') as f:
    f.write(text)
