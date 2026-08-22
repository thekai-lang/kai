kai/
├── Cargo.toml            (workspace)
├── crates/
│   ├── kai-lexer/
│   ├── kai-ast/           # shape only, no logic
│   ├── kai-parser/
│   ├── kai-resolver/      # boleh nyusul sedikit, tapi siapin foldernya
│   ├── kai-tast/          # shape only, no logic — TERPISAH dari kai-ast
│   ├── kai-typecheck/
│   ├── kai-diagnostics/   # {message, span, severity} — ini duluan, bukan belakangan
│   └── kai-codegen/       # cuma boleh depend ke kai-tast, TIDAK ke kai-ast
├── tests/
  └── fixtures/
