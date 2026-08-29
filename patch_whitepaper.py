with open('docs/kai-whitepaper.md', 'r') as f:
    text = f.read()

replacement = r"""### 3.6 Modules

```kai
use support.math;
use support.math.Point;
use domain.entity as e;

fn main() -> int32 {
    let p = Point.create(2, 3);
    let p2 = math.Point.create(4, 5);
    io.println(math.add(p.x, p2.x));
    return 0;
}
```

- `use a.b;` resolves to `a/b.kai` from the **project root**, defined as the directory containing the entry file passed to `kai build`/`kai run` — not the invoking process's working directory. This keeps resolution deterministic regardless of where the command happens to be run from; a future project manifest (not yet designed) may redefine root as its own location, but that's an open question for later, not a v0.0.4 concern.
- **Direct symbol import (v0.0.12)**: `use a.b.Type;` imports the symbol `Type` directly from the `a/b.kai` module. `a/b/Type.kai` is checked first; if it doesn't exist, it falls back to looking for `Type` in `a/b.kai`.
- **Module aliasing (v0.0.12)**: `use a.b as c;` imports module `b` and exposes it under the local alias `c`.
- Path segments `.`, `..`, `/`, `\` are rejected.
- Circular imports are a diagnostic, not a silent stack overflow.
- `public fn` and `public type` are visible through the module alias; plain `fn`/`type` stay module-private. Without `public type`, a struct could never cross a module boundary at all — a module could expose a constructor function but callers would have no way to name or read fields of the type it returns. Both keywords behave identically: `[ 'public' ] 'fn' ...` and `[ 'public' ] 'type' ...`.
- Imports never inject into global scope — always namespace-qualified. **No exceptions, including stdlib.** `println` is always `io.println(...)`; there is no globally-injected builtin form. (This is a deliberate reversal of the v0.4.5 reference implementation, which called `println(msg)` unqualified — that form is not carried forward.)
- **Associated Functions (v0.0.12)**: A function can be scoped to a type by naming it `fn Type.method(...)`. These functions require no `self` parameter (Kai has no implicit `self` or traditional OOP paradigm). They are merely statically resolved functions living in the namespace of a type (e.g. `Type.method()`). The type must be defined in the same module as the associated function.
- **Qualified Types (v0.0.12)**: Type positions accept `Path` annotations, meaning types can be module-qualified (e.g., `let user: auth.User;`).
- **v0.0.4's own tests don't need the stdlib.** Module resolution, qualified calls, `public` visibility, and circular-import detection are all fully exercisable with user-defined modules alone (e.g. a local `support/math.kai` with `public fn add(a: int32, b: int32) -> int32`). The stdlib itself is deferred (originally to v0.0.5, re-anchored to the `kai.toml` manifest design by v0.26, §3.7) — implementing any of it now against types that don't exist yet would just be thrown-away work."""

import re
# Reload text, it might already be patched!
text = re.sub(r'### 3\.6 Modules.*?### 3\.7 Standard library \(built-in, no disk resolution\)', replacement + '\n\n### 3.7 Standard library (built-in, no disk resolution)', text, flags=re.DOTALL)

with open('docs/kai-whitepaper.md', 'w') as f:
    f.write(text)
