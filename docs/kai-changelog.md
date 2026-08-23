# Kai Changelog

This file tracks **compiler implementation** releases (`vX.Y.Z`, what `kai build --version` reports). Specification-level changes are recorded in the whitepaper's own changelog (`kai-whitepaper.md`, `v0.x` line) and cross-referenced here only where they land in code.

---

## v0.0.5.1 — Runtime panics (§10) and ownership leak fixes

A hardening patch over the ownership runtime: the §10 error model becomes real, and the first round of heap-leak audits lands fixes. No new language surface. Release identity is **v0.0.5.1** (git tag + this section); the Cargo workspace version intentionally stays `0.0.5` — semver has no fourth component, and this patch adds no feature surface worth a minor bump.

- **Expression spans in the typed AST** — `TypedExpr` now carries its source span end-to-end, stamped once at typecheck's lowering exit. This is what makes every runtime trap below report a real `at file:line:col` instead of nothing.
- **§10.1 panic infrastructure** — a host `kai_panic(msg, len, file, line, col)` prints the mandated two-line format to stderr and exits `101`; codegen gained `SourceInfo` per module (display path + line-start table) so byte-offset spans resolve to 1-based line/column at emission time, with no source text kept alive.
- **Bounds checks on every indexed access (§10.2)** — array reads, rvalue places, and assignment-place hops all route through one shared `elem_slot` helper: signed pair test (`idx >= 0 && idx < len`) against the header's authoritative length, trap through a panic block carrying the INDEX expression's location, then continue. `for..in` stays check-free by construction — its induction variable is bounded by the same `len` it reads, so a per-iteration guard would be dead weight.
- **Signed arithmetic traps (§10.2, §10.5)** — `+`, `-`, `*` emit LLVM's `llvm.sadd/ssub/smul.with.overflow` for both widths and branch to `integer overflow` panics; `/` and `%` guard zero divisors with their own messages (`division by zero`, `modulo by zero`) plus the one overflowing quotient `MIN / -1`; unary minus on `MIN` traps via checked subtraction. **Deviation-in-spirit, deliberate:** §10.5 names `int32` only, but silent wrapping on `int64` would contradict the policy's own intent ("overflow traps instead of wrapping silently"), so both widths are guarded. Floats stay IEEE (`inf`/`NaN`, never trap).
- **OOM is a Kai panic (§10.8)** — allocation failure and unrepresentable sizes report `out of memory` through `kai_panic` instead of Rust's `handle_alloc_error` abort. Location reads `<runtime>:0:0`: the allocator has no source site of its own.
- **B1 — borrow-position temporaries no longer leak** — owned temporaries consumed only as borrows (call arguments, comparison/projection operands, discarded statement values) had no owner after their statement ended. The ownership pass now materializes each into a hidden `$tmp` local in the current scope, reusing the existing release machinery (block exit AND early returns). `&&`/`||` subtrees are exempt — hoisting would evaluate the right side even when short-circuited; that needs real materialization nodes, deferred to v0.0.6.
- **B3 — `return` inside `for x in <owned temp>` no longer leaks the iterable** — owned iterables now bind to a hidden `$iter` local in the loop's own scope frame, so both normal loop exit and returns from inside the body release through the same frame machinery. The old `iterable_owned` flag path is retired.
- **B4 — double-release detection** — `kai_release` checks the refcount before decrementing: `rc <= 0` prints `kai runtime error: refcount underflow` and aborts rather than corrupting the heap. This guards compiler bugs, not user errors — deliberately distinct from §10 panics.
- **B2 investigated, clean** — the suspected struct-temporary over-retain (`let q = make_user()`) does not exist: zero retains, exactly one scope-exit release.
- **Testing** — 240 tests (+17): IR-textual unit tests per check (guard presence, intrinsic use, message globals, safe paths still compute), six CLI-level tests spawning the real binary for exit code 101 + stderr shape (in-process JIT would take the test runner down), a corpus sweep running every fixture through the pipeline asserting Ok-or-diagnostics (never a panic), shared `assert_golden`/`assert_fails_at` helpers replacing copy-pasted blocks, and ownership-pass tests for the hoisting/loop-frame rewrites.

## v0.0.5 — Ownership runtime

The first release where heap-bearing types exist — and with them, compiler-managed reference counting (§9). Strings land as plain literals; arrays become first-class; `for..in` iterates; every retain/release/move is decided by an explicit compiler phase, never inferred in codegen.

- **`string` type** — plain literals only (`"hello"`); `${...}` interpolation is formally deferred past this version. Exactly five escapes (`\n \t \r \" \\`); any other character after `\` is a lex-phase *error* ("unknown escape sequence"), never silent pass-through.
- **Array literals + indexing** — `[1, 2, 3]` unifies element types; `[]` requires a context type annotation (compile error otherwise: "empty array literal requires a type annotation"); `a[i]` reads and writes with any integer index. Arrays are **unconditionally heap-bearing**, regardless of element type (§9.3).
- **Generalized assignment places** — assignment targets are now `Ident | Place.field | Place[expr]`, arbitrarily chained (`p.a.b = x`, `arr[i].x = y`). Writability follows the ROOT binding only: writable roots are `var` locals and `mut` parameters; projections preserve but never grant permission. The two axes stay independent: writability never inspects types, mutation visibility never inspects mutability.
- **`for..in` loops** — `for v in array { … }`; the loop variable is immutable, element-typed, and BORROWS each element per iteration — the array keeps ownership of everything after the loop. Non-array iterables are a compile error.
- **Ownership resolution phase (§8's "ownership resolution")** — new `kai-ownership` crate between typecheck and codegen. It rewrites the typed AST so every ownership decision is an explicit node codegen reads mechanically: retain markers on borrowed values entering owning slots, scope-exit releases (innermost-first, reverse declaration order), return-with-cleanup that evaluates the value BEFORE unwinding live locals, replacement markers ordering "prepare RHS → release old → store" (self-aliasing safe), and iterable-transfer flags on `for..in`.
- **Retain-on-transfer rule (§9.5)** — reading any binding yields a borrowed reference; only fresh allocations (literals, call results) move free. Entering an owning slot (`let`/`var` init, assignment target, heap-typed `return`, struct-literal field, array-literal element) with a borrowed value inserts a retain — co-ownership, no affine moves.
- **Refcount runtime** — one uniform header `{rc, len, nbytes, payload, dtor}` shared by strings and arrays; refcounts start at 1, `kai_retain`/`kai_release` bump/drop, zero frees. Element destructors run exactly once, inside the final release of the owning header — co-owned arrays never double-release elements.
- **Per-field struct ownership (§9.5)** — a heap-bearing struct (any field recursively heap-bearing) stays a stack aggregate: copying it memcpy's the fields and retains each heap field individually; releasing releases each field individually. Whole-struct refcounting was rejected by design — it would silently turn unrelated scalar fields into reference semantics. Scalars stay copy-semantics even next to string siblings.
- **String equality** — `==`/`!=` compare CONTENT via the runtime, never pointer identity, and are borrow operations (§9.7): same text from different allocation paths compares equal, guaranteed by tests.
- **Testing** — 223 tests: lexer/parser/typecheck unit tests for the new surface (escapes, empty-array rule, indexing rules, writability matrix through indexed places), ownership-pass unit tests (retain classification, release orderings, cleanup-carrying returns), e2e JIT tests proving aliasing safety and cross-path content equality, and golden-IR fixture `v0005`.

## v0.0.4 — Module system

The first multi-file release: programs are module trees rooted at an entry file.

- **`use` imports** — `use a.b;` loads `<root>/a/b.kai`, where `<root>` is the ENTRY FILE's directory (never the process CWD — deterministic builds); the alias is the last path segment (`math.add(…)`). Modules load once each, so diamond imports are fine.
- **Circular import detection (§7 exit criterion)** — revisiting a module that is still on the DFS stack is a compile error reporting the whole chain (`cyclic import: mod.a -> mod.b -> mod.a`), never a stack overflow; the diagnostic points at the offending `use` site in the importing file.
- **Per-module namespaces (§3.6)** — unqualified names resolve ONLY inside the declaring module: imports never inject into any scope. Qualified references go through aliases to PUBLIC declarations only — `public fn` / `public type` qualifiers gate access, and violations report the qualified path (`function `core.secret` is not public`). Types and functions stay separate namespaces; import aliases form a third.
- **Qualified struct literals** — `alias.Point { … }` builds public structs from other modules; field rules unchanged.
- **Two-branch callee rule** — a call like `x.f(…)` where `x` is not an import alias is treated as value semantics first (field access lowers normally), then rejected as a non-direct call with a targeted diagnostic instead of miscompiling as a module reference.
- **Entry contract scoped** — `main` must live in the entry module; an imported public `main` does not make a program runnable.
- **Symbol qualification** — module-scoped declarations get `module.name` LLVM symbols (and `%module.Name` struct types), so same-named declarations in different modules coexist in one binary; entry-module names stay bare for JIT.
- **Multi-file diagnostics** — every diagnostic carries its file; the reporter renders each against its OWN source (caret lines always match the named file), including errors raised inside imported modules during typecheck.
- **Testing** — loader unit tests (pre-order loading, diamond reuse, cycle chains, missing modules, per-file attribution), resolver/typecheck unit tests for visibility and namespace isolation, codegen symbol-collision tests, and golden-IR fixture `v0004`: a three-file project (public struct through alias, two same-named private helpers) whose JIT run returns 31.

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
