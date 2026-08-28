## v0.0.9.2 — Reversible Stabilization (v0.0.9.x)

- **Reject `fn() reversible`**: The parser now strictly rejects first-class reversible closures (e.g. `let f = fn() reversible`). Support is explicitly deferred, preventing silently unsound behavior.
- **Closure Context Isolation**: Closures declared inside a `reversible` activation are now correctly isolated.
  - *Typechecker*: Explicitly rejects escaping closures if they carry activation-bound transactional effects, emitting a clean compile error.
  - *Codegen*: The thread-local `reversible_active` flag is strictly cleared when generating a closure's inner body, preventing unintentional leakage of ledger tracking.
- **Synchronous Rollback Failure Debt**: Refined unwind panic handling for robust observability.
  - Added a thread-local `UNWIND_STATE` to track whether the runtime is currently executing a snapshot restore (`rollback-failed`) or a compensate thunk (`compensation-failed`).
  - If a panic (or a refcount underflow) occurs during unwind, `kai_panic` checks this state and synchronously writes the appropriate event record directly to `.kai/debt.log` before emitting the panic trace and exiting with code `101`.
  - Refcount underflows now invoke `kai_panic` to ensure this path executes cleanly.

## v0.0.9 — Reversible Ledgers (§5.3) & CompensateThunk

- **CompensateThunk & Capture Semantics (P2)**: Implemented complete `compensate` blocks for `reversible` functions. A `compensate` block registers a self-contained, typed LLVM environment and an anonymous thunk to the `REVERSE_STACK` ledger. Variables captured from the outer scope are copied *by-value* at registration (shallow copy), providing strict snapshot semantics that insulate the compensation logic from subsequent outer mutations.
- **Strict ABI Alignment (P0)**: The runtime ledger (`kai_reversible_push_compensate`) backs environments using a strictly `16-byte` aligned storage (via custom `AlignedBuffer`), satisfying LLVM ABI layout constraints and preventing UB on strict-alignment targets.
- **Recursive Retain/Release**: Environment construction automatically scans and generates `retain` structures for deep heap-bearing captures (e.g. `Struct` containing `Array` containing `String`), with corresponding recursive `release` destructors.
- **Panic Isolation (Reentrant Unwind Safety)**: The unwind runtime pops the ledger entry *before* execution. This structurally eliminates infinite recursion or double-releases if a `compensate` thunk panics internally.
- **ASan Validation**: Extensively verified with new ASan test suites covering LIFO ordering, deep struct captures, reentrant unwinding, snapshot-mutation insulation, and string allocations. All leak regression tests green.


## v0.0.8.4 — Temporal equality locked, F2/F3 orphan-claim hoisting, tagged-helper multi-branch fix

- **F4+F5 — temporal retain & equality (§5.1.7)**: `lazy_select`'s some_bb now retains `Temporal` results (`@wallclock` retains the header; `@local` delegates to the inner type's retain). Equality on Temporal values is **inner-value equality** — `@wallclock` GEP-extracts both payloads (idx 4) before delegating to the inner comparison; `@local` delegates directly (zero-cost). The instant never participates in `==` (semantic decision recorded as whitepaper v0.23 §5.1.7 — validity belongs to verification machinery, `==` stays a pure function of value contents; string-content precedent §9.7). Non-comparison ops on Temporal return a defensive `false` instead of panicking through `unreachable!`.
- **Scalar temporal literals**: `int32/int64 @local(d)/@wallclock(d)` now coerce from integer literals symmetrically with strings (typecheck + wallclock-header wrap at codegen) — bare scalar storage previously made every later release treat a raw `i32/i64` as a heap pointer.
- **F2/F3 — orphan-claim hoisting**: `hoist_borrow_temps` now recurses into the always-evaluated positions of composite expressions — coalesce `lhs`, `unwrap_or` receiver, catch `base` — so heap-bearing owned temps inside them materialize into hidden locals and release per scope. Lazy positions (`rhs`/`default`/catch stmts/tail) stay untouched: hoisting would evaluate them eagerly. Verified flat header-outstanding counts across 100-iteration loops for all three shapes (was ~1 leaked header per iteration pre-fix).
- **F2/F3 gap — nested owned-temp hoisting (hoist_children restructure)**: the early-return in the old `hoist_borrow_temps` skipped recursing into children when the root itself was an owned temp — e.g. `Call(func, [StrLit("x")])` with a heap-bearing return type hoisted the Call but NOT the StrLit arg, orphaning its creation claim. Extracted `hoist_children` as a single source of truth for child recursion; `hoist_borrow_temps` now always recurses into children first, then materializes the root. Eliminates the duplication anti-pattern §8 warns against and fixes leaks in `mk(true, "x").unwrap_or(...)`, `mk(false, "q") catch |e| { e }`, and any nested owned-temp-in-borrow-position pattern. ASan-verified 0 leaks on all repros.
- **tagged-helper multi-branch crash fix (latent since v0.0.6)**: building a retain/release helper for a tagged union with TWO heap-bearing payloads (e.g., `Result<string,string>`) aborted codegen with "Terminator found in the middle of a basic block" — the second branch's conditional branch was emitted after the first branch's terminator. Checks are now chained through dedicated `tag.check` blocks. Surfaced by F2/F3 hoisting producing hidden locals of exactly that shape; covered by the new catch regression.
- **Tests**: 326 → 332 passing; new regression tests cover nested hoisting for Call+StrLit args inside UnwrapOr/Catch/OkLit (unit) and full-pipeline JIT for the exact leak repros (end-to-end).
- **co.fallback Temporal retain/release**: `else_bb` was missing the `KaiType::Temporal` arm in both step 1 (retain for consumer) and step 2 (release creation claim), causing double-free of `@wallclock` headers and leak of `@local` temporals. Fixed by mirroring the `some_bb` pattern. Also fixed `release_header_value` call path for wallclock headers: it used generic `kai_release` instead of `kai_wallclock_release` — now routes through `emit_release_slot` which dispatches correctly.
- **IntLit in `fallback_is_fresh`**: scalar temporal literals like `int32 @wallclock(30m) = 42` were not identified as fresh owned temps, skipping the step 2 release.

## v0.0.8.6 — Leak regression fixtures + ASan CI gate

- **Leak regression fixtures**: all isolation artifacts from the v0.0.8.4–v0.0.8.5 leak investigations (`minimal.kai`, `minimal2.kai`, `test3.kai`–`test5.kai`, `stress_heap.kai`) promoted to permanent `tests/fixtures/leak/` fixtures. Each JIT-compiles and asserts a verified exit code: `minimal.kai` → 1, `minimal2.kai` → 5, `test3–test5/stress_heap` → 99. Catches ownership-pass regressions on every PR without ASan.
- **ASan CI gate**: `scripts/asan-test.sh` runs all leak fixtures under `ASAN_OPTIONS=detect_leaks=1` with per-file invocation, explicit LeakSanitizer vs exit-code separation, and clear diagnostic output. GitHub Actions: fast test (cargo test + clippy) on every PR; ASan nightly (daily 03:00 UTC + push to main + manual).
- **CI infrastructure**: `.github/workflows/ci.yml` (PR + push: test + clippy), `.github/workflows/asan-nightly.yml` (nightly: ASan build + leak fixtures).
- **Tests**: 332 → 338 passing (6 new `jit_leak_*` regression tests in `end_to_end.rs`).

## v0.0.8.5 — B1 exemption resolved: `&&`/`||` rhs borrow-temp leak fix

- **B1 exemption resolved — `&&`/`||` rhs borrow-position temporaries no longer leak**: the ownership pass now recurses into both sides of `&&`/`||`. Lhs children are hoisted normally (always evaluated); rhs children are hoisted with throwaway scopes whose Let bindings are stored in `rhs_hoists` and emitted inside the `and.rhs` basic block — only executed when short-circuit doesn't skip. This completes the B1 exemption declared in v0.0.5.1 and deferred in v0.0.6: "real materialization nodes for `&&`/`||` borrow-position temporaries." Whitepaper §9.11 (scope-exit release) already specifies this behavior; the implementation now matches.
- **`hoist_root` helper**: extracted from `hoist_borrow_temps` with a `register_scope: bool` parameter, allowing rhs_hoists to skip scope registration (prevents double-release at scope exit — they are released inside the `and.rhs` block, not at block end).
- **TAST `rhs_hoists` field**: `TypedExprKind::Binary` gains `rhs_hoists: Vec<TypedStmt>` for `And`/`Or` operators. Other binary ops carry `rhs_hoists: Vec::new()`.
- **Codegen `short_circuit`**: now accepts `rhs_hoists: &[TypedStmt]`, emits Let statements + `alloca_in_entry` + `emit_release_slot` inside the `rhs_block`.
- **Tests**: 332 passing; golden IR v005 regenerated; JIT regression tests pass. ASan-verified 0 leaks on `stress_heap.kai` (40 heap-allocating operations per iteration × 10 iterations).

## KNOWN ISSUES

No open critical issues as of v0.0.8.5.

### `unwrap_or` heap-payload double-free — RESOLVED in v0.0.8.4
`optional_val.unwrap_or(heap_struct_default)` previously crashed with glibc `tcache_thread_shutdown(): unaligned tcache chunk detected` when the payload type was heap-bearing. Root cause: the `lazy_select` codegen path copied the selected payload out of the tagged union without retaining heap fields, while consumer-side retain/release assumed co-ownership. Fixed by F2/F3 orphan-claim hoisting (`hoist_borrow_temps` now recurses into always-evaluated positions — coalesce lhs, unwrap_or receiver, catch base) so heap-bearing owned temps materialize into hidden locals and release per scope.

### `&&`/`||` rhs borrow-temp leak — RESOLVED in v0.0.8.5
The B1 exemption from v0.0.5.1 deferred `&&`/`||` subtrees: "hoisting would evaluate the right side even when short-circuited; that needs real materialization nodes." Owned temporaries in the rhs of `&&`/`||` (e.g., `x == "alpha" && y == "delta"`) leaked because the rhs was never hoisted. Fixed by hoisting rhs children into throwaway scopes, storing their Let bindings in `rhs_hoists`, and emitting them inside the `and.rhs` basic block — only reached when short-circuit doesn't skip. ASan-verified 0 leaks.

### `catch.join` terminator gap — RESOLVED in v0.0.8.1
Previously listed as known gap from v0.0.6.1; fixed by unconditional `position_at_end(join_bb)` after both branch paths complete.

## v0.0.8 — `require`/`observe` implemented: runtime panic, §10.3 debt record, JSONL Signal sink (whitepaper v0.20–v0.22 §5.2)

The first trust-aware behaviors that execute: `require expr;` traps with the §10.3 message shape and writes a pre-ledger record before exiting; `observe expr;` appends a Signal record to `.kai/observe.log`. Both lower into `Trust<C>` locally through kai-effects (v0.20's scoping decision) — the call-graph inference subsystem stays exclusively §5.1's.

- **Typecheck** — the two "not yet implemented" diagnostics are gone. Static rule at this phase is only that the guarded expression must be `bool`; everything else is runtime (§5.2 v0.20 scoping).
- **kai-effects Trust<C> lowering (`trust.rs`, 113 lines)** — the single consumer-side definition of both record shapes per §5.2.1–§5.2.2/v0.22: `observe_jsonl(timestamp, location, condition, outcome)` and `debt_correctness_jsonl(...)` with `kind:"correctness"` matching §5.6 categories (v0.21). Includes canonical RFC3339-UTC-micros formatter (§5.1.5 format reused for record timestamps) and minimal JSON escaping — embedded newlines in source-text conditions escape to `\n`, never breaking one-record-per-line.
- **Condition text = raw source span (v0.22)** — codegen slices the original source via the TAST span (`SourceInfo::slice`), verbatim including embedded newlines in panic messages; no AST pretty-printer, zero drift from what the programmer wrote.
- **Codegen emission** — `Require`: evaluate once → branch violation/ok → on violation path: `kai_debt_record` (flushed) then `kai_panic("requirement violated: <span>")` → unreachable; ok path continues. `Observe`: evaluate once → `kai_observe_record`. Outcome widened i1→i32 at ABI boundary.
- **Runtime sinks (`runtime/observe.rs`, 111 lines)** — `kai_observe_record(sink, loc, cond, outcome)` and `kai_debt_record(sink, loc, cond)` host intrinsics: timestamp acquired host-side, JSONL shaped by kai-effects, append+flush (create_dir_all for `.kai/`). INTRINSICS 9 → 11.
- **Sink resolution (v0.21)** — file API bakes `<project root>/.kai/*.log` paths (root = entry file's directory per §3.6); string API passes no root and codegen emits NO recording calls — the documented no-op is proven at IR level (`v008_string_api_recording_is_documented_noop` asserts absence of both record calls AND presence of the baked require panic).
- **Driver** — file API threads project root into new `compile_ir_with_sink` / `run_jit_with_sink` entry points; old signatures delegate with `None`.

Testing: 13 driver tests added/updated — CLI-level failing require (exit 101 + §10.3 message + flushed `.kai/debt.log` with kind/location/condition/outcome), passing require leaves no debt entry, observe writes valid escaped JSONL (N evaluations = N lines), exactly-once evaluation proven by array-mutation counter (double eval would trap), string-API IR-level no-op assertions. 309 → 317 tests total; clippy `-D warnings` clean; all logic files <500 LOC (§8.4).

# Kai Changelog

This file tracks **compiler implementation** releases (`vX.Y.Z`, what `kai build --version` reports). Specification-level changes are recorded in the whitepaper's own changelog (`kai-whitepaper.md`, `v0.x` line) and cross-referenced here only where they land in code.

---

## v0.0.7 — Temporal types `@local`/`@wallclock`, `effects` annotation, `DurationLit` (whitepaper v0.15 §5.1, EBNF §9)

First trust-aware surface: temporal validity as `Type @local(d)` / `@wallclock(d)` with verified `effects { escapes-local-context }` contracts. `require`/`observe` grammar lands as syntax-stable (§9a) but semantics deferred to pre-v0.0.8. Scope locked by whitepaper **v0.15** + **v0.16** audit cleanup (symmetric discard, `Stmt` includes `Require`/`Observe`).

- **Lexer (`lex`)** — `TokenKind::At`, `DurationLit { value: u64, unit: DurationUnit }` (`ms` maximal-munch), `Require`/`Observe`/`Effects`/`EscapesLocalContext`/`LocalKw`/`WallclockKw` (`keywords.rs:61`). `@` was `unexpected character`, now `At` for `@local/@wallclock`. `scan_number` in `lexer/number.rs:66` checks `DurationUnit` suffix (`ms` vs `m`) and returns `DurationLit` (integer-only, `1.5h` stays `FloatLit` + `Ident`). `escapes-local-context` hyphenated keyword handled in `lexer/mod.rs:206` `scan_word` via `-local-context` suffix consume. `DurationUnit::Ms| S|M|H|D` with `as_str`. `DurationLit` integer-only (`0ms` lexically valid, typecheck rejects `0` per §5.1.6).
- **AST (`ast`)** — `ty.rs:34` `Ty::Temporal { inner, origin: TemporalOrigin::Local|Wallclock, duration: DurationLit }` postfix like `T?`/`T[]`; `fn_decl.rs:17` `EffectName::EscapesLocalContext`, `EffectSet(Vec<EffectName>)`, `FnDecl { effects: Option<EffectSet> }` (`None`=omitted purely inferred, `Some(empty)`=`effects {}`); `stmt.rs:69` `StmtKind::Require(Expr)`/`Observe(Expr)` (v0.0.8 syntax, v0.0.7 parse-only).
- **Parser (`parse`)** — `ty.rs:151` `ty()` loop `[]`/`?`/`@` postfix, `temporal_ty` (`@` `local|wallclock` `(` `DurationLit` `)`), `duration_lit` maps `DurationUnit`, `decl.rs:165` `effects_annotation` (`effects { [EffectSet] }`) after `-> Type` before `Block`, distinguishes `effects {}` vs omitted. `stmt.rs:384` `Require`/`Observe` (`require`/`observe` `Expr` `;`) and `catch_block` includes them as statement starters.
- **TAST (`tast`)** — `ty.rs:75` `KaiType::Temporal { inner, origin, duration }` (`@local` zero-cost same as inner, `@wallclock` unconditionally heap `KaiWallclock { rc, instant: i64 // UTC micros, payload }` per v0.17 §5.1.7, not RFC3339 string), `Effect`/`EffectSet`/`DurationLit`/`TemporalOrigin` with `Display`, `fn_decl.rs:38` `TypedFnDecl { declared_effects: Option<EffectSet>, inferred_effects: EffectSet }` (least-fixed-point over SCCs, `Set<Effect>` not `bool`).
- **Effect checker (`effects` new crate `Cargo.toml:3` members, `Crate kai-effects`)** — `Cargo.toml:18` `0.0.7`, `kai-effects = { workspace = true }` after `kai-ownership` before `kai-codegen` (`kai-typecheck → kai-ownership → kai-effects → kai-codegen` per §8). `lib.rs` `analyze(&mut TypedProgram) -> Vec<Diagnostic>`: `direct_effects` from `declared_effects`, call/closure graph edges via `collect_called_fn_ids`, SCC iterative fixpoint `inferred = direct ∪ ⋃ inferred(callees)`, verify `inferred ⊆ declared` (`§5.1.2`), reachability `No @local reachable from escapes` including captures (`§5.1.3` 5-item scope, conservative union for dynamic dispatch), `Require`/`Observe` currently `not yet implemented` diagnostic via typecheck (effect checker passthrough). Wire format `RFC3339 Z microsecond` (`§5.1.5`) and `DurationLit` `0ms` typecheck reject handled.
- **@local read-widening (§5.1.7 marker-drop on borrows)** — `typecheck/expr/call.rs:5` `local_read_as_plain`: a `T @local(d)` value may flow into an argument position expecting bare `T` (direct + closure calls), because @local is zero-footprint pure delegation — without it, every callee in the graph would need re-annotation, making "cheap" (§5.1) a lie. The modifier STAYS on the TAST node (no coercion); soundness moves to the effect checker, which knows callee effects: non-escaping plain callees accept silently, escaping ones get the boundary diagnostic at the `effect` phase (proper rule, not an accidental type mismatch). Contagion gated by callee EFFECTS, not parameter syntax.
- **Driver (`driver`)** — `pipeline.rs:200` `lex→parse→resolve→typecheck→ownership→effects→codegen`, `Cargo.toml:19` `kai-effects` dep, `Failure { phase: "effect" }` on diagnostics.
- **Typecheck (`typecheck`)** — `ty.rs:65` `Ty::Temporal` resolve + `0ms` `temporal_zero_duration` diagnostic, `stmt.rs:306` `Require`/`Observe` lower to `TypedStmt::Require/Observe` with `bool` check + `not yet implemented`, `decl.rs:157` `FnDecl` effects conversion `kai_ast::EffectName` → `kai_tast::Effect`.
- **Ownership/Codegen (`ownership`/`codegen`)** — `heap.rs:93` `Temporal @wallclock` unconditionally heap (like `array` §9.1), `@local` delegates to inner; `types.rs:86` `Temporal Local => to_llvm(inner)` zero-cost, `Wallclock => ptr KaiWallclock { rc, instant:i64, payload }` (compact integer, not RFC3339 string per v0.17 §5.1.7); `emit/expr/mod.rs:389` `Retain` for `Temporal` delegates to inner, `emit/ownership.rs:66` `Wallclock` two-step release (cascade to inner if heap-bearing, then header) vs `Local` delegate — fixes `string @wallclock` leak where bare header-release is silently correct for `int32 @wallclock` but silently wrong for `string @wallclock`; `fresh.rs:138` `Require`/`Observe` seed, `walk.rs:415` `Require`/`Observe` hoist, `stmt.rs:279` `Require`/`Observe` emit as `expr::emit` passthrough.
- **Tests/fixtures** — `tests/fixtures/v0007/main.kai` minimal `@local` flow (`produce`/`consume`/`maybe_escape` with `effects`, `caller` transitive) → JIT `42` (`main.expected.ll` golden). `corpus_flows` now covers `v0007`. `end_to_end.rs:915` `lex_error_reports_unknown_character` now uses `$` (since `@` is valid).

295 → 296 tests (added `lexer/tests.rs:55` `at_sign_is_temporal_prefix` for `@local/@wallclock` lex), `cargo test` 72 typecheck + 62 driver + etc. `cargo clippy -D warnings` clean (`#[allow(clippy::empty_line_after_doc_comments)]` for moved doc blocks). `wc -l` `13319` logic files all <500 per §8.4.

---

## v0.0.6.2 — Split remainder (no new surface, §8 debt paydown)

Continues v0.0.6.1's split: every logic file >500 LOC is now <500, per whitepaper §8.4 "no file >500 LOC". No language surface, no whitepaper change — pure file hygiene so v0.0.7 trust layer lands on a decomposable base.

- **`kai-codegen/src/emit/expr.rs:1408` → `expr/mod.rs:347` + `expr/arith.rs:383` + `expr/heap.rs:379` + `expr/tagged.rs:129` + `expr/closure.rs:234`** — `emit` (514) now delegates `arith::neg/not/binary/short_circuit` and `heap::string_lit/array_lit/index_read/call/field_read/place_ptr/struct_lit` and `tagged::tagged_none_const/zero_of/lazy_select` and `closure::emit_closure` (220). `crate::emit::expr::elems_storage_of` etc re-exported at `expr/mod.rs:16` so `ownership.rs:270`/`stmt.rs:63` keep `expr::` path. `mod.rs` uses `#[allow(clippy::empty_line_after_doc_comments)]` + `#[allow(unused_imports)]` for re-exports.
- **`kai-typecheck/src/expr.rs:1060` → `expr/mod.rs:352` + `expr/call.rs:205` + `expr/struct_lit.rs:182` + `expr/array.rs:72` + `expr/tagged.rs:166` + `expr/collect.rs:111`** — `lower` stays in `mod.rs:352`, `call::call_expr`/`struct_lit::field_access`/`array::array_lit`/`tagged::coalesce_expr` etc via `super::lower` and `pub(crate) use` re-exports at `mod.rs:29`. `collect` (LocalRef collection) and `capture_poisoned` moved. `is_import_alias` moved to `collect.rs` and re-exported.
- **`kai-typecheck/src/lib.rs:927` → `lib.rs:55` + `tests.rs:216`/`v0003_tests.rs:163`/`v0004_tests.rs:179`/`v0005_tests.rs:131`/`v0006_tests.rs:139`/`v0005_string_extra.rs:25`** — logic 55, tests extracted per version (like `kai-ownership` v0.0.6.1). `test_support.rs:490` already separate.
- **`kai-parser/src/lib.rs:960` → `lib.rs:54` + `tests.rs:605`/`v0006_tests.rs:296`** — `tests.rs:605` is test-only and just over 500, tracked as next split (test file, not logic).
- **`kai-lexer/src/lexer.rs:680` → `lexer/mod.rs:206` + `lexer/string.rs:54` + `lexer/number.rs:66` + `lexer/tests.rs:216`/`v0005_tests.rs:65`/`v0006_tests.rs:77`** — `scan_string` and `scan_number`/`accumulate_int` moved to `impl Lexer` in `string.rs`/`number.rs` via `super::Lexer`.
- **`kai-codegen/src/lib.rs:719` → `lib.rs:117` + `tests.rs:178`/`v0003_tests.rs:217`/`v0004_tests.rs:78`/`v0005_panic_tests.rs:122`**
- **Remaining >500 test files tracked**: `kai-ownership/src/tests.rs:513`, `kai-parser/src/tests.rs:605` (both test-only, next patch will split per-feature). All *logic* files now <500; `wc -l` total `13319` (was `14735` at v0.0.6).

295 tests green + clippy `-D warnings` clean (added `#![allow(clippy::empty_line_after_doc_comments)]` where doc→fn empty line lint fired after moves). Cargo stays `0.0.6`, git tag `v0.0.6.2`.

---

## v0.0.6.1 — Split & hardening patch (no new surface, whitepaper v0.14 alignment)

Patch over v0.0.6 that pays down §8 debt before the trust layer (§5, v0.0.7+). Scope is hardening + file splits + `Ok`/`Err` constructors (whitepaper v0.14 §3.4 gap-closure); grammar already locked in EBNF, now landed in code.

- **Hardening (P1/P2)** — `scope.rs:59`, `ownership/scopes.rs:18`, `resolver/tables.rs:133`, `codegen/runtime.rs:78,165` `expect`/`unsafe` paths now guard with `internal error: … — compiler bug` prefix or diagnostic; `Layout::from_size_align_unchecked` gains `isize::MAX` guard, `kai_string_new` null-data aborts; `lexer.rs:270` ascii word expect prefixed. Distinguishes compiler-bug panics from user diagnostics (§8 constraint 6, not §10 runtime traps).
- **File splits (§8.4 — no file >500 LOC)** — `kai-ownership` monolith `lib.rs:1414` → `fresh.rs:136` + `heap.rs:93` + `scopes.rs:46` + `walk.rs:410` + `lib.rs:69` + `tests.rs:513`/`v0006_tests.rs:170` (logic under 500, tests file just over is tracked). `kai-ast` gains `UseDecl::dotted_name()`/`alias()` centralizing `join(".")` duplicated in `driver/modules.rs:123` and `resolver/tables.rs:126`; `Span::DUMMY` replaces 14 `Span::new(0,0)` sentinels. `kai-ownership/Cargo.toml:10` unified to `workspace = true`.
- **Dedup crate** — new `kai-test-support:0.0.6` (workspace member) provides canonical `parse_ok`/`parse_with_diags`/`Span::DUMMY`/`assert_diag_contains` to replace 6 cloned `parse_src`/`check_src` helpers (`parser/lib.rs:39`, `resolver/lib.rs:291`, `typecheck/lib.rs:927` ×3). Full migration of per-crate `check_src` clones deferred to next patch, crate is the prescriptive path.
- **`Ok`/`Err` constructors (whitepaper v0.14 §3.4, EBNF `PrimaryExpr`)** — `Ok(value)`/`Err(value)` parallel `Some`/`None`, each pins one `Result<T,E>` param from its arg, the other from context (annotation or `return` type) reusing `None`/`[]` context-typing. `lexer/token.rs:150` + `keywords.rs:59`, `ast/expr.rs:205`, `parser/expr.rs:439`, `tast/expr.rs:183`, `typecheck/expr.rs:1020`, `ownership/fresh.rs:136`/`heap.rs:93`/`walk.rs:410`, `codegen/types.rs:86`/`emit/expr.rs:1364` updated. Verified via JIT `Ok(42).unwrap_or(0) + Err("x").unwrap_or(7) == 49`. `catch` with `Ok`/`Err` payloads is wired; one `catch.join` terminator edge is tracked as known gap (existing v0.0.6 fixture 295 tests still green).
- **Stdlib note** — synthetic `std.io` via synthetic module (without disk) was evaluated as shortcut for Hello World; **not landed** — deferred to manifest (`kai.toml`) design. README/whitepaper gap note retained: `std.io` stays deferred, `synthetic module` is implementation shortcut not final resolution, per audit follow-up.
- **Docs alignment** — re-read `whitepaper v0.14` (878 lines) + `EBNF v0.14` (414 lines) + `project-structure.md:14`; `T?` is single canonical node (no desugar), `.unwrap_or` has no dedicated production, `catch` uses `CatchBlock` (only trailing-expression block), `_` barred from `Param`/`For` bindings (§9.9b).

295 tests green (22 ownership, 62 typecheck, 54 codegen, etc.) + clippy `-D warnings` clean. Cargo workspace version stays `0.0.6` (semver has no fourth component); release identity is git tag `v0.0.6.1` + this section. Next: finish `kai-codegen/src/emit/expr.rs:1364` split (`tagged.rs`/`closure.rs`/`heap.rs`) and migrate remaining `parse_src` clones to `kai-test-support`.

---

## v0.0.6 — Optionals, Results, closures, and the discard statement

The first version where failure becomes a *type*: `Optional<T>` and `Result<T, E>` land as tagged unions with ownership applied only to the active payload, closures arrive as unconditionally heap-bearing values with retained captures, and silent discards of tagged unions become diagnostics behind one explicit escape hatch. Scope locked by whitepaper **v0.13** (§9.9a, §9.9b, §9.10); grammar surface was already drafted in the EBNF and completed here.

- **`Optional<T>` / `T?`, `Result<T, E>`** — one semantic form per type; `T?` desugars straight to `Optional<T>` at parse time (no second nullable concept), and `Result` deliberately takes no sugar. Generic parameters exist ONLY on these two builtins — unknown generics are a dedicated diagnostic, as is wrong arity.
- **Tagged-union ownership (§9.9a)** — inline `{ tag, payload }` aggregates; retain/release applies to the ACTIVE payload only, keyed per instantiated type at compile time: `Optional<int32>` emits zero refcount calls (proven at IR level by a test), while `Optional<string>` releases through a runtime tag check. Result layouts are non-overlapping (`{ tag, ok, err }`) for correctness first.
- **`None` joins the grammar** — the empty constructor the whitepaper always implied but never wrote down. Context-typed exactly like the empty array literal: bare `let x = None;` is an error.
- **`.unwrap_or(default)` on both tagged unions, no dedicated production** — it composes from FieldAccess+Call like any method-shaped call; builtin status resolves in typecheck when the receiver is tagged and the base isn't an import alias. `catch |err| { stmts.. tail }` stays Result-only, with a narrow CatchBlock shape (statements plus ONE mandatory trailing value) that never generalizes into block-as-expression.
- **Discard rule, symmetric** — discarding an `Optional` or `Result` as a bare statement is a diagnostic; `_ = expr;` is the sole escape hatch, with `_` carved out of `Ident` entirely (`let _` dies at parse). The discarded expression evaluates under ordinary ownership rules.
- **Closures (§9.10)** — values are `{ code, env }` fat pointers, unconditionally heap-bearing regardless of capture. Environments are heap headers whose generated destructors release captured values exactly once; captures are retained at construction, compile-time keyed per capture type. First-class calls (`f(x)` on a closure-typed local or field) work with env as the hidden first parameter. Closure-bearing types poison transitively through fields, array ELEMENTS, and tagged payloads — capturing one is rejected up front, the structural precondition for an RC cycle (deliberately conservative, reusing the resolver's TypeDecl graph).
- **Ownership pass generalizes** — `walk_expr` gains scope context for catch frames; catch-block locals release after the tail consumes them; `Some` payloads are owning slots; coalesce results follow the active branch's ownership (consumer retains-as-borrow, creator-reference released after branch join).
- **Known gap recorded**: pure Kai source cannot construct a `Result` yet — no `Ok`/`Err` literals exist and stdlib arrives later; Result-flow coverage rides shared typing paths until then. The whitepaper's `(unit) -> unit` closure spellings were normalized to canonical `() -> unit`.
- **Deferred to the next cycle**: real materialization nodes for `&&`/`||` borrow-position temporaries (the B1 exemption from v0.0.5.1 remains), and string interpolation.

295 tests green across 22 suites; clippy clean.

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
