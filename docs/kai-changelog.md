# Kai Changelog

This file tracks **compiler implementation** releases (`vX.Y.Z`, what `kai build --version` reports). Specification-level changes are recorded in the whitepaper's own changelog (`kai-whitepaper.md`, `v0.x` line) and cross-referenced here only where they land in code.

---

## v0.0.3 — Structs, parameters, calls, field access

First aggregate value type, and with it real function interfaces.

- **Struct types** — `type Name = { field: Type; … }` declarations anywhere in the file (order-independent: a struct may reference any other, cycles are a *compile error* reported as a path — `A → B → A`); fixed-size, stack-allocated, copy semantics throughout.
- **Typed function signatures** — parameters (`fn add(a: int32, b: int32) -> int32`) with distinct fn/type namespaces (`Point { … }` vs `Point(…)` never collide at parse time); direct calls only — anything that is not a bare declared name is rejected with a targeted diagnostic rather than miscompiled.
- **By-value parameter passing (§9.3)** — every argument is copied into the callee; `mut p: Point` gates writes to the callee's own copy with zero ABI difference from an unannotated parameter, locked end-to-end by a JIT test where caller state must survive a mutating call.
- **Field access & places** — `value.field` chains of arbitrary depth as rvalues; assignment targets extend from plain bindings to field paths (`seg.start.y = 20;`, compound forms included) lowered as root-alloca + getelementptr chains; reads copy.
- **Struct literals** — `Name { field: expr, … }`; fields may appear in any order (the type checker reorders them into declaration order), missing/duplicate/unknown fields are compile errors; literals are banned *bare* inside `if` conditions (NO_STRUCT_LITERAL — the `{` would read as a block), parentheses lift the ban.
- **Strict same-type field rules** — field reads/writes type-check against the declaration exactly (no literal widening through fields); mismatched field init or assignment names the field and both types.
- **Codegen** — two-pass emission (all signatures first, then bodies) so calls resolve regardless of definition order, recursion included; structs become named LLVM literal types; parameters land as entry-block allocas seeded from their incoming values.
- **Parser robustness fix** — a malformed struct-declaration field used to spin the parser forever (expect_* recovery consumes nothing); recovery loops now detect no-progress and skip a token, locked by a regression test.
- **Testing** — golden-IR fixture `v0003` (nested structs, mutating by-value call, nested field-place write) plus JIT tests for call composition and parenthesized-literal conditions.

## v0.0.2.1 — Hardening

Robustness pass over the v0.0.2 core; no new surface syntax.

- **Parser recursion budget** — every recursive expression production funnels through one entry point guarded by a depth budget (`MAX_EXPR_DEPTH = 256`). Pathological nesting no longer overflows the native stack: 50,000 nested parentheses now produce a single `expression nested too deeply` diagnostic at the parse phase instead of a crash.
- **Unary chains are budgeted too** — prefix operators (`-`, `!`) parse iteratively, but each still consumes budget, so downstream phases that recurse over AST depth stay safe on inputs like `-----…1`.
- **Poisoned recovery nodes** — parser error recovery now yields `ExprKind::Invalid` / `TypedExprKind::Invalid` instead of valid-looking placeholders (previously a fake `IntLit(0)` could leak into later phases). Typecheck lowers it defensively with its own diagnostic; codegen emits LLVM `undef`. Invariant enforced by tests: a placeholder is never mistaken for real code.
- **Scoped overflow reporting** — each independent over-budget expression reports once; recovery regions reset the reported flag so two deep expressions yield two diagnostics, not one.
- **Stable duplicate-binding ids** — same-scope redeclaration keeps resolving references to the ORIGINAL binding. `Locals::declare` returns `DeclareOutcome::{Fresh, Duplicate(original)}` and the id counter does not advance on duplicates; the internal `u32::MAX` sentinel hack is gone.
- **Entry-block allocas** — every stack allocation is emitted at the top of the function's entry block, even for bindings declared inside nested blocks or branches (LLVM-friendly placement). Locked by an IR-invariant test.
- **Unit-function termination tests** — empty unit body and bare `return;` both fall through to designed behavior: emitted as `ret void`.
- **Lexer literal diagnostics** — malformed numerics get precise messages instead of misleading "unexpected character" errors: `1.` → *"float literal needs a digit after `.`"*, `.5` → *"number literals must start with a digit"*, `1..2` consumes both dots as one recovery region (exactly one diagnostic).
- **Docs/versioning scheme clarified** — whitepaper carries an unversioned *implementation note* distinguishing spec versions (`v0.x`) from compiler versions (`vX.Y.Z`); §10.2 arithmetic panic checks recorded as a known divergence (specified, not yet emitted by codegen).

## v0.0.2 — Bindings, primitives, arithmetic, control flow

First feature release on top of the pipeline skeleton.

- **Bindings** — `let` (immutable) and `var` (mutable) with type inference from the initializer; optional type annotations (`let big: int64 = 5;`) that widen integer literals to fit.
- **Assignment statements** — plain (`x = e`) and compound (`x += e` lowering to read-modify-write); assignment is statement-only, never an expression, with an explicit assignable-place rule (identifier only in this release) rejected at parse phase.
- **Primitive types** — `int32`, `int64`, `float64`, `bool` (+ `unit` internally); literals are range-checked against their inferred width (`2147483648` reports "does not fit" with the literal's span); int/float never mix implicitly.
- **Arithmetic & comparison** — `+ - * / %` with locked precedence chain; signed comparisons `< > <= >= == !=`; modulo restricted to integers.
- **Unary operators** — unary minus and logical NOT, applied to any expression; negative literals fold at parse time so codegen never sees an unrepresentable positive constant under `-`.
- **Control flow** — `if` / `else if` / `else` with strictly `bool` conditions (enforced), nested blocks, and block scoping: shadowing allowed across scopes, rejected within one scope.
- **Boolean logic** — `&&` / `||` with short-circuit evaluation verified end-to-end (a division-by-zero guard behind `false && …` is provably skipped) and precedence `&&` > `||`; `!x` via XOR-fold.
- **Definite-return analysis** — non-unit functions must return on every path; `if` without `else` does not satisfy it (typecheck phase, with span).
- **Codegen layout** — locals become named allocas + load/store; branch merging uses phi nodes; both-branches-return leaves an explicitly dead fallback block so modules verify.
- **Testing scale-up** — golden-IR fixture for the full pipeline plus JIT execution tests per feature (precedence, mutation, shadowing, short-circuit, int64 widening, floats); multi-error reporting within typecheck (several diagnostics with spans/carets in one run).

## v0.0.1 — Full pipeline skeleton

Minimum end-to-end slice: source text in, executed program out.

- **Workspace** — Cargo workspace, phase-per-crate: `kai-lexer` → `kai-parser` → `kai-ast` → `kai-resolver` → `kai-typecheck` → `kai-tast` → `kai-codegen` → `kai-driver`, with shared `kai-diagnostics`.
- **Language surface** — exactly one shape compiles: `fn main() -> int32 { return <int-literal>; }`.
- **Diagnostics-first** — errors are `{ message, span, severity }` from day one, with caret rendering; phases stop at the first failing stage and report which phase failed (`lex`, `parse`, `resolve`, `typecheck`).
  - unknown characters report at lex phase,
  - syntax errors (e.g. missing `;`) stop before resolve,
  - a missing `main` is a resolve-phase error,
  - out-of-range int32 literals report at typecheck with the literal's range highlighted.
- **Backend** — inkwell/LLVM 22: module emission + verification, golden-file IR test locking the exact output, and JIT execution via `kai run` (exit codes asserted in tests).
- **CLI** — `kai build <file.kai> [-o out.ll]` and `kai run <file.kai>`, plus `--version` / `--help`.
