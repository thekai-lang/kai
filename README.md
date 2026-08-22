# Kai

A trust-aware programming language. Compiler pipeline in Rust:

```
kai-lexer → kai-parser → kai-resolver → kai-typecheck → kai-codegen (LLVM via inkwell)
                ↓              ↓               ↓
            kai-ast       (kai-tast)    kai-diagnostics
```

- `kai-ast` / `kai-tast` — shape-only IR definitions, no logic.
- `kai-codegen` depends only on `kai-tast`, never `kai-ast`.
- Specs live in [`docs/`](docs/): whitepaper, EBNF grammar, project structure.

## Build

Requires Rust 1.85+ and LLVM 22 (`llvm-config` must be on `PATH`).

```sh
cargo build
cargo test
```

## Status

Pre-implementation scaffold per `docs/kai-project-structure.md`.
