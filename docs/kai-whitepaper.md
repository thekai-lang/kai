# Kai
### A trust-aware programming language

**Status:** Draft v0.25 — pre-implementation specification
**Purpose:** Freeze scope before writing any compiler code. Nothing described here is authoritative until it appears in this document. Feature ideas that arise during implementation go into an `IDEAS.md` backlog, not into the compiler.

**Amendment process:** Small additions (new syntax sugar, clarifying rationale) may be edited directly. Anything touching §2 (principles), §4 (non-goals), or introducing a new Trust kind beyond §5.0's taxonomy must first exist as an entry in Appendix A, be discussed explicitly, and only then be promoted into the main body — never patched in ad hoc during implementation.

**Changelog**
- **v0.26** — Amended §3.7's stdlib gate. Records honestly, for the first time in this document, that the §3.7 boundary ("Everything above this line must be fully specified, implemented, and tested before Section 5 begins") was crossed implicitly: stdlib was formally deferred to v0.0.5 at v0.6, never landed, and was re-deferred to the `kai.toml` manifest design by only a one-line note in the v0.0.6.1 changelog — after which v0.0.7/v0.0.8/v0.0.9 (the §5 trust-aware layer) all shipped without it, and the whitepaper itself was never updated. This entry closes that gap. It distinguishes §3's *semantics* (types, ownership, modules, control flow — genuinely complete and tested before §5, satisfying the gate's intent) from §3.7's *stdlib surface* (pure API functions over those already-complete types, introducing no new type-system, ownership, or codegen rule §5 could unknowingly depend on). The former is why the deferral was safe; the testing record corroborates it — §3.6's own note that v0.0.4 needs no stdlib, and every v0.0.5–v0.0.9 suite running against user-defined fixtures alone, with zero stdlib dependency. **The gate is re-anchored, not deleted:** the stdlib becomes a hard requirement not merely for the §5 test suites but for any §5 feature (`dsl sql`/`dsl api`/`kai sync`) to be exercised by a real, non-fixture Kai program — concretely anchored to the `kai.toml` project-manifest design that v0.0.6.1 already deferred it to, which §5.6's per-project `kai debt` severity overrides likewise presuppose. In test/fixture-only form, §5 features continue not to require it. Amendment touches §3 only — no §2/§4/§5 taxonomy change, so the §2 amendment process's Appendix-A route is not implicated.
- **v0.25** — Formalized §5.3's transactional mechanism as the **pre-mutation Place snapshot**, replacing the earlier "automatic inverse" framing. Transactional rollback is snapshot-and-restore (capture the Place's value before the write; restore it during reverse-order unwind) — never a symbolically derived operator inverse, which cannot be guaranteed exact (integer rounding, float64 IEEE, compound traps). Scope widened (signed off) from arithmetic-only to all assignments to writable Places per §9.3's Place-model consistency; heap-bearing snapshots participate in §9's ownership model, holding an independent retain sufficient to restore safely. The `.rollback()` explicit API is removed from v0.0.9 scope — panic-triggered unwind is the sole reversal path. §10.4, §5.0's table, and §7's roadmap row updated to match. Applies the §2 amendment process: entry drafted in §5.3, discussed, then promoted; no §0.0.9 implementation before §5.3.1's ownership-safe snapshot semantics are locked.
- **v0.24** — Added Appendix A entry: memory hardening framework proposal (3 pillars: CI regression fixtures, ASan gate, panic-path memory test). Separates immediate work (v0.0.8.6: move leak repros to `tests/fixtures/`, ASan CI gate) from deferred framework (v0.0.12: heap profiling, static lifetime, fuzzing, concurrency). Documents the 4-level production standard (P0–P3). Explicitly does NOT touch §2/§4/§5 — Appendix A only, per amendment process.
- **v0.23** — Locked temporal equality semantics in §5.1.7 (found unspecified during v0.0.8.4 stabilization, when `==` on `@wallclock` values was first compiled): **equality on Temporal values is inner-value equality — the instant participates only in verification machinery (§5.1's still-open runtime checks) and never in `==`.** Rationale: folding the instant into `==` would define equality as issuance-time identity, breaking `==`'s fundamental property as a pure function of value contents — `t == t` could flip to false after expiry or when two copies were created microseconds apart; validity status is orthogonal to value identity and belongs to the verification machinery (`Trust<C>`, §5.0/§5.1), not `==`. Precedent: string `==` compares content, never allocation identity (§9.7). Gate stays strict — both sides must be `Temporal` with identical origin, duration, *and* inner type; mixed temporal/plain is rejected at typecheck. Any future need for claim-level identity (same text *and* same issuance time) must surface as a separate accessor/verification API, never as `==` overloading.
- **v0.22** — Locked the last open detail for v0.0.8: condition text in `require`'s panic message and `observe`'s JSONL records is the raw source-text span (§5.2.1), never an AST pretty-print — reuses existing `Diagnostic`/TAST span infrastructure, guarantees the rendered text always matches what the programmer wrote verbatim, and needs no new pretty-printing machinery. §5.2's semantics are now fully specified end to end.
- **v0.21** — `require` violations now write a concrete pre-ledger record before panicking (§10.3): `.kai/debt.log`, JSONL, one line per violation, `kind` field matching §5.6's existing debt categories so v0.0.12's aggregation needs no format translation — mirrors `observe.log`'s sink design (§5.2.2) rather than inventing a second pattern. Locked the sink-location question for both logs when compiling via the string API (no project root): a documented no-op, explicitly not CWD — falling back to CWD would have directly contradicted §3.6's rejection of CWD for module-root resolution (same non-determinism argument applies identically here).
- **v0.20** — §5.2 (`require`/`observe`) formalized to §5.1's rigor ahead of v0.0.8: `require` gets a full `Trust<C>` lowering table (§5.2.1) plus a synchronous, exactly-once evaluation rule; `observe`'s recorded shape and sink are locked (§5.2.2) — a pluggable interface with exactly one v0.0.8 implementation, a local append-only JSONL file (`.kai/observe.log`), consistent with §2.1's offline-by-default principle; full `kai debt` dashboard integration deferred to its already-scheduled v0.0.12 slot. Precisely scoped what §5.1's apparatus `require`/`observe` do *not* need: the call-graph inference subsystem (`EffectName`/`EffectSet`, SCC/fixpoint, `effects { ... }` contracts) — but not `kai-effects`/`Trust<C>` lowering itself, which both constructs still go through (locally, with no graph to traverse) so `kai debt` and `@override` continue to operate on `Trust<C>` uniformly per §5.0/§8.
- **v0.19** — New §5.1.3a closes the Appendix A gap found during v0.0.7's own boundary-test suite: `T @local(d)` may be read as plain `T` at an argument position without being stripped from the TAST — the safety decision moves entirely to the effect checker (which callee effect set applies), consistent with §8's phase order (typecheck cannot know a transitively-inferred effect set yet). Non-escaping callees now accept `@local`-tracked arguments silently; escaping callees still reject them, now via the correct boundary-specific diagnostic rather than an incidental type mismatch. v0.0.7 tag: `80d9410`.
- **v0.18** — v0.0.7's core safety property (boundary rejection) confirmed proven by three distinct compiler-verified negative tests: direct `@local`→escaping-function rejection at the effect phase, exact-type rejection at typecheck when passing `@local` to a plain (unmodified) parameter, and transitive rejection across a two-hop call chain via fixpoint inference (both hops correctly flagged, not just the direct one). v0.0.7 is complete. New Appendix A item, surfaced by the second test: passing a `@local`-typed value to a plain-`T` parameter is rejected via exact-type matching regardless of whether the callee actually escapes — open question whether this should also block *non-escaping* functions (which would make `@local` impractical across any real call graph, contradicting its zero-runtime-footprint design intent) or whether an explicit, always-safe strip conversion (`T @local(d) → T`) is needed. Unresolved, needs a decision before `@local` sees real multi-function use.
- **v0.17** — New §5.1.7, prompted by a real leak found during v0.0.7 implementation (codegen treated `Temporal` as a release no-op). Locks the two Origins as genuinely asymmetric, not two branches of one shape: `@wallclock` is unconditionally heap-bearing (array's precedent — a header exists regardless of inner type), `@local` is pure zero-footprint delegation to its inner type (more minimal than `Optional`/`Result`'s conditional delegation — no wrapper at all, compile-time-only). Header's internal instant representation locked as a compact integer (UTC microseconds since epoch), explicitly not the RFC 3339 string form — that's a serialization-boundary contract only (§5.1.5), never the in-memory shape. Release specified as two-step whenever the inner type is heap-bearing (cascade into payload, then release the header) — the exact case a bare unconditional header-release silently gets wrong. Same asymmetry applies uniformly to `heap_bearing` queries, retain, and closure-environment dtors, not just the release path that surfaced it.
- **v0.16** — Post-implementation audit cleanup, no semantic changes to already-shipped behavior. Struck a stale Appendix A note that still framed `Optional`'s discard policy as undecided — it was resolved symmetrically with `Result` at v0.13 (§9.9a) and has been implemented and tested since v0.0.6.2 (295 passing tests); the note simply hadn't been updated to match. (Grammar doc: same cleanup for the matching stale open item, and `require`/`observe`'s grammar — stable since v0.2 — separated out of the v0.0.7 section into its own v0.0.8 section, since roadmap §7 already scoped it there and bundling it with §5.1's newly-locked material risked implying its semantics were formalized to the same rigor, which they are not yet.)
- **v0.15** — §5.1 fully formalized ahead of v0.0.7 implementation, closing both remaining Appendix A blockers. Boundary defined mechanically as an effect (`escapes-local-context`) rather than a keyword list, with an explicit construct table (§5.1.1). Effect inference specified as transitive over the call graph, least-fixed-point over SCCs for cycles, attached to the function/closure (never inferred from a parameter type) (§5.1.2). Declared `effects { ... }` annotations specified as a verified contract (`inferred ⊆ declared`, checked, never blindly trusted) — and shown to independently reduce to the `Trust<C>` shape (§5.0), which the document records as validation of that abstraction rather than coincidence. Closures locked as first-class call/effect-graph nodes carrying a `{ effects, captures }` summary (§5.1.3), with an explicit, deliberately bounded v0.0.7 scope (direct call, known closure invocation, closure-as-argument, closure-returned-or-stored, conservative union for dynamic dispatch) and one general reachability invariant replacing the earlier "direct arguments only" framing. `@wallclock → @local` fixed as having no conversion path at all in v0.0.7, not merely "non-implicit." Wire format locked: canonical RFC 3339, UTC-only, microsecond precision, mandatory `Z` suffix (§5.1.5). `DurationLit` grammar locked: integer-only, fixed unit suffixes (`ms`/`s`/`m`/`h`/`d`), lexical validity kept separate from semantic (duration-value) validity (§5.1.6).
- **v0.14** — `Ok(value)`/`Err(value)` locked as `Result<T,E>`'s language-level constructors, parallel to `Some`/`None` (§3.4) — closes a real gap where `Result` was receive-only (usable as a stdlib return type) but never constructible in user code, which would have undercut it being genuinely symmetric to `Optional` (§9.9a already made the *representation* symmetric; construction needed to match). Each constructor infers only one of the two type parameters from its argument; the other requires context (annotation or enclosing `fn`'s declared return type), reusing the same context-typing mechanism as `None` and empty array literals rather than inventing a separate rule. §3.4 also tightened: `??` is explicitly lazy (short-circuit, matching `&&`/`||`'s existing discipline); `T?`/`Optional<T>` clarified as parsing directly to one canonical type node with no desugaring pass between them; `catch`'s block clarified as the language's only trailing-expression block, deliberately not a general block-expression feature; explicit non-goal note that Optional/Result/closures are the only parametric typing in the language, not evidence of a general user-facing generics mechanism. Grammar: `.unwrap_or(...)`'s dedicated production removed (resolved at typecheck via receiver-type + field-name, same parser-meaning-agnostic discipline as qualified calls); `catch` now uses a dedicated `CatchBlock` production instead of ordinary `Block`; `_` explicitly barred from `Param` and `ForStmt`'s binding position, not just `let`/`var`.
- **v0.13** — v0.0.6 scope locked (§9.9a, §9.9b, §9.10): `Optional<T>`/`Result<T,E>` represented as tagged payloads, ownership applying only to the active branch and only when its instantiated type is heap-bearing — one mechanism, generalizing the per-field (§9.8) and per-element (§9.9) pattern rather than a separate implementation per type. `T?` fixed as canonical sugar for `Optional<T>` (no second semantic form); `Result<T,E>` gets no postfix sugar. `.unwrap_or()` now explicit for both `Optional` and `Result`; `catch` stays `Result`-only. Discarding `Optional`/`Result` as a bare statement is now a diagnostic symmetrically for both types (previously `Result`-only, `Optional` was an open Appendix A question) — shipped alongside its escape hatch in the same version: `_ = expr;`, the sole explicit-discard statement, with `_` carved out of the `Ident` grammar entirely (reserved, never a normal binding name). Closures fixed as unconditionally heap-bearing regardless of capture (mirrors array's existing unconditional-heap rule) — never an optimization-driven exception. Closure reference cycles are rejected at v0.0.6, not deferred: a closure may not capture any value whose type is or transitively contains a closure type, checked structurally by extending the existing `TypeDecl` cyclic-struct DFS (§3.3) rather than building new alias analysis — deliberately conservative over unsound.
- **v0.12** — Two small v0.0.5 gaps closed ahead of implementation (§3.4): empty array literals require an explicit type annotation (`let arr: int32[] = [];` — no inference from nothing, consistent with the existing explicit-over-implicit stance on literal widening); string escape sequences fixed to `\n \t \r \\ \" \0`, with an unrecognized escape producing a lex-phase diagnostic naming the bad sequence — `\$` deliberately excluded since `${` isn't special until interpolation itself lands (still deferred, Appendix A).
- **v0.11** — v0.0.5's string scope locked to plain literals only; `${...}` interpolation formally deferred (§9.7, new Appendix A entry) — it needs evaluation-order, value-to-string-conversion, and temporary-ownership decisions that don't exist yet, and folding it into v0.0.5 would blur a version whose focus is ownership, not string formatting. String `==`/`!=` specified as content comparison, explicitly never pointer identity, and specified as a borrow operation (reads both operands, retains/releases nothing) — consistent with field access and array indexing already being borrows (§9.8, §9.9); pointer-equality fast paths remain a legal *implementation* optimization underneath this rule as long as it stays unobservable.
- **v0.10** — §9.3 gains the formal two-axis invariant: `is_writable(place) = is_writable(root(place))` (Axis 1, mutability) is fully independent of `visibility(place) = mutation_regime(type_of(root(place)))` (Axis 2, stack-local vs. heap-shared) — neither axis inspects the other. Two explicit anti-patterns recorded (writability must never depend on element type; a container's ownership category must never be inferred from its element type). Retain-before-release ordering (previously stated as a new-looking rule) reworded to make clear it's §9.4's existing `var`-reassignment ordering rule extended to a new destination kind (array element), not a parallel rule — keeps the v0.0.5 amendment small. Added an explicit `var`/`let`/`mut`-param/plain-param × array/struct test matrix to v0.0.5's exit criteria (§7), proving the two axes stay independent in practice.
- **v0.9** — Generalized the mutability gate (§9.3) into a single `Place` model: root binding determines writability, field access and array indexing are both just projections that inherit it — one rule instead of a separate rule per construct. Array indexing added to `Place` (grammar) for v0.0.5. Called out explicitly that arrays are unconditionally heap-bearing per §9.1 (unlike `Optional`/`Result`, which are conditional) — so `mut arr: int32[]` is caller-visible even though `int32` is a stack type, unlike an all-scalar `mut` struct parameter (D1); this asymmetry is easy to misjudge from element type alone. Generalized §9.4's existing "prepare replacement before releasing old value" ordering rule to any `Place` replacement, not just plain `var` reassignment — named explicitly as safety-critical for array-element replacement, where the RHS can alias the destination slot being replaced (`arr[0] = arr[0]` must not read freed memory).
- **v0.8** — Formalized, ahead of v0.0.5 implementation, how structs with heap-bearing fields are copied/released (§9.5, new subsection): per-field retain/release, never whole-struct refcounting. This was already implicit in §9.8's existing wording, not a new decision — now made explicit with the mechanism, its codegen implications, and why the whole-struct-refcount alternative was rejected (it would silently give reference semantics to unrelated stack-type fields the moment any sibling field is heap-bearing, contradicting §9.1's unconditional stack-copy-semantics claim).
- **v0.7** — `Diagnostic` shape gains `file` (§8 constraint 6), surfaced as a real gap during v0.0.4 implementation planning: `span` alone is ambiguous once a compilation can span multiple source files, and §10.1's panic format already implied a file-qualified location without the base shape carrying one. `file` is nullable/absent in the single-file phases (v0.0.1–v0.0.3) and populated from v0.0.4 onward. (Grammar doc, not whitepaper: `StructLit`'s head generalized from a bare `Ident` to `QualifiedName` — dotted, reusing `ModulePath`'s shape — so `math.Point { ... }` parses without a separate "qualified struct literal" node; confirmed no new AST node is needed for qualified calls either, since `math.add(...)` already composes from the existing `Call`/`FieldAccess` rules — the parser stays meaning-agnostic about module-vs-value, per open item #6, matching the parser/resolver boundary already enforced elsewhere in this project.)
- **v0.6** — v0.0.4 scope locked: `public type` added alongside `public fn` (without it, structs couldn't cross module boundaries at all — a real gap, not a stylistic choice); project root for `use` resolution defined as the entry file's directory, not the invoking process's CWD, for deterministic resolution; stdlib (§3.7) formally deferred to v0.0.5 since every signature in it depends on `string`/arrays; noted that v0.0.4's own test suite needs no stdlib at all (user-defined modules suffice). §3.1's Hello World example annotated as the target shape, not literally achievable before v0.0.5 (it needs both modules and `string` at once).
- **v0.5** — Inserted a new v0.0.5 ("Ownership runtime") into the roadmap between the module system (v0.0.4) and Optional/Result/closures — string, array literals + indexing, `for..in`, and actual retain/release enforcement (§9.4–9.9) all land together, since they're the first point any heap-bearing type exists. Everything previously numbered v0.0.5–v0.0.11 shifts to v0.0.6–v0.0.12; every cross-reference in this document is updated accordingly. Fixed §9.5's retain-rule enforcement claim, previously (incorrectly) attributed to v0.0.3 — v0.0.3 has zero heap-bearing types active (all structs are stack-only per §9.1), so retain literally cannot be exercised there; the claim now sits on the new v0.0.5. `mut` on a stack-type parameter is formalized as local-copy-permission only, with zero ABI difference from an unannotated parameter — one rule ("`mut` grants write access through the binding"), two consequences depending on whether the type is stack (invisible to caller) or heap (visible through the borrow, from v0.0.5 onward). §9.3's example fixed: no more bare `fn show(s: string) { ... }` (missing the mandatory `->`, and using a type with no version slot until this change) — replaced with a `Point`-based example matching v0.0.3's actual scope. v0.0.3's scope is now explicit about field read *and* write access (write gated by `mut`), and adds a compile-time cyclic-struct-definition check (DFS over the `TypeDecl` dependency graph, diagnostic reports the cycle path) — indirection/boxing to legitimately break such cycles is not yet designed and is called out as a known gap, not a silent omission. Discarding a non-`unit` call result is explicitly allowed without diagnostic in v0.0.3 (no correctness risk for scalars/structs); revisited once `Result` exists (new v0.0.6) where silently discarding it would violate §2.3.
- **v0.4** — Absorbed core-language semantics that were decided during v0.0.1/v0.0.2 implementation but not yet recorded here, so the whitepaper stays the single source of truth rather than the compiler's own changelog competing with it: block scoping/shadowing rules, definite-return analysis, integer literal widening, and assignment-as-statement-not-expression (§3.2a, new). §3.6 now states explicitly, with no exceptions, that stdlib calls are always namespace-qualified (`io.println(...)`, never a bare `println`) — the v0.4.5 reference implementation's unqualified form is not carried forward.
- **v0.3** — `kai debt` reframed explicitly as a *projection* of Trust state, not an independent fifth feature (Debt = unresolved Trust violations/degradation; Signal carries no debt). Reversibility formally split into two Trust subtypes in §5.0's table — Transactional (`state_after + inverse = state_before`) and Compensatable (`state_after + compensator ≈ acceptable_new_state`) — while `reversible` stays the single user-facing keyword. Added architectural principle: syntax lowers into one uniform `Trust<C>` IR; the effect checker and `kai debt` operate only on that IR, never on originating syntax (§5.0, §8 constraint 8). Added Decay taxonomy to Appendix A as a proposed, not-yet-adopted direction.
- **v0.2** — Reframed §5 around a formal Trust model (§5.0). `assume` split into `require` (correctness, always panics) and `observe` (telemetry, never panics — not a Trust, a Signal). `@duration` split into `@local` (flow-relative, compiler-checked) and `@wallclock` (real-clock, runtime-checked; mandatory once a value crosses a process boundary). `reversible` split into transactional mutation (auto-invertible, "rollback" reserved for this) and `compensate` (manual, non-atomic, for external side effects — "rollback" is no longer used for this case). §9 clarified as compiler-internal guarantee, not a developer-facing model.
- **v0.1** — Initial draft: core language, trust-aware layer (assume/@duration/reversible/dsl), ownership model, runtime error model.

---

## 1. Motivation

Every non-trivial backend depends on things it does not fully control:

- A database schema that migrates without every caller being updated.
- A third-party API whose contract changes underneath you.
- A token, session, or lease that is valid only for a window of time.
- An invariant a function silently assumes ("this list is never empty") that nobody ever wrote down.
- A mutation whose effects nobody planned how to undo.

None of these are compiler errors today. They surface as runtime crashes, silent data corruption, or 2am incidents — long after the code that caused them was written and forgotten.

Kai's position: **trust is not free, and trust has a shape.** Every time code depends on something outside its own guaranteed correctness — an external schema, a deadline, an assumption, an irreversible action — that dependency should be a visible, trackable thing in the language, not an implicit hope.

Kai does not try to eliminate this dependency (that's impossible). It tries to make it **impossible to depend on something silently.**

---

## 2. Design principles

These principles are the filter for every future feature decision. If a proposed feature violates one without a strong justification, it does not belong in Kai.

1. **Static and offline by default.** `kai build` never requires a network connection or a live database. A minimal program compiles and runs with zero external dependencies. Anything that needs to reach outside the project (checking a live schema, syncing an OpenAPI spec) is an explicit, separate command (`kai sync`), never an implicit side effect of `kai build`.

2. **Trust is declared, not assumed.** If code depends on an external contract (DB schema, API, event payload), a temporal guarantee (token validity), a correctness invariant, or a reversible action, that dependency is named in the language — not left as a comment or tribal knowledge. All four are instances of one formal structure (§5.0), not four unrelated features.

3. **Drift is a first-class, queryable concept.** Whenever a declared trust becomes questionable — a schema changed, a token expired unexpectedly, a correctness invariant was violated, an override has gone unverified — it accumulates as **debt**, inspectable in one place (`kai debt`), not scattered across logs, incident reports, and migration tools.

4. **Escape hatches are visible, not silent.** When the outside world is wrong (a spec is inaccurate, a correctness invariant needs to be forced), Kai allows overriding it — but the override is owned, dated, and re-checked automatically when the underlying source changes.

5. **Progressive disclosure.** None of the above is required to write `fn main() -> int32 { return 0; }`. Trust-aware features (`require`, `@local`/`@wallclock`, `reversible`, `dsl sql`, `dsl api`) are opt-in layers on top of an otherwise ordinary, small, statically typed language. A newcomer should be able to ignore all of Section 5 and still be productive.

6. **One compiler, phased internally.** The compiler implementation itself must mirror this document's structure: lexer, parser, resolver, type checker, effect checker, codegen — as separate, independently testable modules. A single file growing past a few hundred lines is treated as a design smell, not a thing to leave for later.

---

## 3. Core language (v0.0.1 – v0.0.6 scope)

This section is intentionally boring. It is Rust/Go-adjacent on purpose — the interesting ideas live in Section 5, not here.

### 3.1 Hello world

```kai
use std.io;

fn main() -> int32 {
    io.println("Hello, Kai!");
    return 0;
}
```

**Version note:** this exact program isn't runnable until v0.0.5 — it needs both the module system (v0.0.4) and `string`/stdlib wiring (v0.0.5). It's shown here as the target shape of the language, not as a v0.0.1 milestone; v0.0.1's actual minimum is `fn main() -> int32 { return 0; }` (§7).

### 3.2 Types

```kai
let count: int32 = 42;
let ratio: float64 = 3.5;
let ready: bool = true;
let name: string = "Kai";
```

Primitive families:
- Stack: `int32`, `int64`, `float64`, `bool`, `unit`
- Heap-bearing: `string`, arrays, closures, structs containing any of the above

`int`/`float` are **not** separate types — they are aliases (`type int = int32; type float = float64;`) so casual code doesn't need to think about width until it matters.

### 3.2a Bindings, control flow, and literal rules

These are genuine language-semantic rules (not implementation trivia) — they determine which programs are valid Kai, so they belong here rather than only in the compiler's own release notes.

- **`let` vs `var`.** `let` is immutable (cannot be reassigned); `var` is mutable. Both require an initializer (§9.2) — type is inferred from it, or may be given explicitly (`let big: int64 = 5;`).
- **Integer literal widening.** An explicit wider type annotation widens the literal to fit its declared type (`let big: int64 = 5;` — `5` widens to `int64`). This only applies to literals with an explicit target type; `int` and `float` values never mix implicitly, and there is no implicit narrowing.
- **Assignment is a statement, not an expression.** `x = value;` and compound forms (`x += value;`, etc.) cannot appear nested inside another expression (`x = (y = 5)` is not valid Kai) — this avoids the classic `if (x = 5)` vs. `if (x == 5)` foot-gun by construction, not by lint.
- **Block scoping and shadowing.** A `let`/`var` may shadow a binding from an *enclosing* scope. Redeclaring the same name within the *same* scope is rejected — this is not shadowing, it's a duplicate declaration error.
- **`if`/`else` conditions must be `bool`.** No implicit truthiness for other types.
- **Definite-return analysis.** A function with a non-`unit` return type must return on every reachable path. An `if` without a matching `else` does not, on its own, satisfy this — the compiler traces control flow, not just the presence of a `return` statement somewhere in the body.
- **Discarding a call's return value is allowed, silently, in v0.0.3–v0.0.5.** `foo();` where `foo` returns a scalar or struct carries no correctness risk at this stage, so no diagnostic is produced. From v0.0.6, both `Result` and `Optional` require a diagnostic when discarded silently — symmetric, not just `Result` (§9.9a): `Optional` now carries real semantic information (`None` vs `Some`), so silently ignoring it is exactly as dangerous as swallowing a `Result`'s error channel. `_ = expr;` (§9.9b) is the explicit escape hatch, introduced in the same version.

### 3.3 Structs

```kai
type User = {
    id: int32;
    name: string;
}

let user = User { id: 1, name: "Kai" };
```

**Cyclic struct definitions are a compile error.** A struct that references itself, directly or through a cycle of other struct types (`type A = { b: B }` / `type B = { a: A }`), has infinite size with no indirection to break it — structs are stack-allocated and fixed-size. This is detected as a DFS over the dependency graph formed by `TypeDecl` field types; the diagnostic reports the cycle path (`A → B → A`). **Indirection/boxing to legitimately express self-referential types (linked lists, trees) is not designed yet** — this is a known, explicit gap, not a silently missing feature. Until a boxing mechanism exists, such types simply cannot be expressed in Kai.

### 3.4 Arrays, Optionals, Results

**Version note:** `int32[]`/array literals and indexing land at v0.0.5 (Ownership runtime — the first point any heap-bearing type exists, §9.4–9.9). `string` lands at the same version, for the same reason — as a **plain literal only**; `${...}` interpolation is explicitly deferred past v0.0.5 (§9.7). `Optional`/`Result`/closures land at v0.0.6. The examples below use all of these together for readability; none of them are available before v0.0.5, and `Optional`/`Result` specifically not before v0.0.6.

**Empty array literals require an explicit type annotation.** `let arr: int32[] = [];` is valid; `let arr = [];` is a typecheck error ("cannot infer element type of empty array literal") — this isn't new inference machinery, it's the same explicit-over-implicit stance already taken for integer literal widening (§3.2a): the compiler never guesses a type from nothing to fill in.

**String escape sequences (v0.0.5): `\n`, `\t`, `\r`, `\\`, `\"`, `\0`.** No others — an unrecognized escape (`\q`, for example) is a lex-phase diagnostic, consistent with the precision-first lexer diagnostics already established (malformed numeric literals get a specific message, not a generic "unexpected character"). `\$` is deliberately not part of this set: since `${` isn't special in v0.0.5 (interpolation is inactive, §9.7), there's nothing for it to escape yet.

```kai
let values: int32[] = [1, 2, 3];

let maybe_name: string? = Some("Kai");
let absent: string? = None;               // payload type from the annotation, same
                                           // mechanism as an empty array literal (§3.4, v0.0.5)
let fallback: string = maybe_name ?? "unknown";

let parsed: Result<int32, string> = str.parse_int("42");
```

**`Ok(value)`/`Err(value)` construct a `Result` directly — the language-level constructors, parallel to `Some`/`None` for `Optional`.** Without these, `Result` would be receive-only (usable as a return type from stdlib calls) but never constructible in user code, which defeats the point of it being a first-class, symmetric-to-`Optional` type (§9.9a already made the representations symmetric — the construction API needs to match). Each constructor only pins down *one* of `Result<T, E>`'s two type parameters from its argument — the other has no information source and needs context, exactly the same mechanism as `None` and empty array literals (§3.4, §9.9a): the enclosing `let`/`var` annotation, or the function's declared return type when used directly in a `return` statement.

```kai
fn parse(s: string) -> Result<int32, string> {
    if s == "" {
        return Err("empty input");   // E=string inferred from the argument; T=int32 from the fn's declared return type
    }
    return Ok(42);                    // T=int32 inferred from the argument; E=string from the fn's declared return type
}

let r: Result<int32, string> = Ok(42);        // T from the argument, E from the annotation
let bad: Result<int32, string> = Err("nope"); // E from the argument, T from the annotation
```

A bare `let r = Ok(42);` with no annotation and no return-type context is a typecheck error requiring an explicit annotation — same rule as `None`, never a silent guess.

**`??` is lazy — the right-hand side is only evaluated when the left-hand side is `None`.** Not eager evaluation of both sides followed by a select: `maybe_name ?? expensive_call()` never runs `expensive_call()` if `maybe_name` is `Some`. This is the same short-circuit discipline already established and tested for `&&`/`||` (v0.0.2 changelog verified this end-to-end for booleans); `??` follows the identical branch-based codegen pattern rather than evaluating both operands unconditionally.

**`T?` and `Optional<T>` are two surface syntaxes for the exact same internal type — there is no desugaring *pass* between them.** Both are parsed directly into one canonical type node (`KaiType::Optional(T)`); the parser never constructs a separate "sugar" representation for `T?` that a later phase translates into the generic form. This is a slightly stronger claim than "T? desugars to Optional<T>" might suggest — there's no intermediate step to point to, because there's only ever been one node to begin with.

**Only `Optional`, `Result`, and closures use parametric typing, and it's built-in, not a general language feature.** There is no user-facing generic syntax (`MyBox<T>`, `type Pair<A, B> = ...`) anywhere in the roadmap — these three are special forms the compiler knows about natively, not evidence of a general generics mechanism a Kai program could extend. (Same reasoning as why arrays use postfix `T[]` rather than `Array<T>`, §3.2: avoiding any syntax that implies a capability that doesn't actually exist.)

**Change from v0.4.5:** `??` is reserved for `Optional` only. `Result` requires an explicit, non-silent unwrap:

```kai
let value: int32 = parsed.unwrap_or(0);
let value: int32 = parsed catch |err| { io.eprintln(err); 0 };
```

**`catch`'s block is the one and only place in Kai where a block produces a value as a trailing expression.** `{ io.eprintln(err); 0 }` ends with `0` and no semicolon — that's the recovered value, not a statement. This is deliberately **not** a general block-expression feature: function bodies, `if`, and `while` bodies are all still pure statement sequences with no trailing-expression form. Generalizing "blocks can produce values" everywhere would be a language-wide semantic change; `catch` gets a narrow, dedicated grammar shape (`CatchBlock`, grammar §6) instead, precisely because its entire purpose — "recover a value of the `Ok` type" — needs a value to come out of it and nothing else in the language currently does.

**`.unwrap_or(default)` works on both `Optional<T>` and `Result<T,E>`** — it's the same combinator either way ("if the value isn't there, use this fallback"), so there's no reason to restrict it to one:

```kai
let a: int32 = maybe_name_len.unwrap_or(0);   // Optional<int32>
let b: int32 = parsed.unwrap_or(0);            // Result<int32, string>
```

**`catch |err| { ... }` is `Result`-only.** It plays a different semantic role — "intercept and handle an error" — that doesn't have an `Optional` analog; `Optional`'s absence case has no error to intercept, only `??`'s fallback-value role, which it already covers. `Optional` deliberately does not get a `catch` form.

Rationale: silently discarding an `Err` via the same operator used for `None` contradicts Kai's core principle (§2.3) that failure must be visible. This is a deliberate, permanent divergence from the old syntax.

### 3.5 Functions and closures

```kai
fn greet(user: User) -> string {
    return "Hello, ${user.name}!";
}

fn make_printer(prefix: string) -> (string) -> void {
    return fn(value: string) -> void {
        io.println(prefix + value);
    };
}
```

**Change from v0.4.5:** closure type is written `(string) -> void`, not `fn(string) -> void`, to avoid two `fn` tokens stacking visually in a signature.

### 3.6 Modules

```kai
use support.math;

fn main() -> int32 {
    io.println(math.add(2, 3));
    return 0;
}
```

- `use a.b;` resolves to `a/b.kai` from the **project root**, defined as the directory containing the entry file passed to `kai build`/`kai run` — not the invoking process's working directory. This keeps resolution deterministic regardless of where the command happens to be run from; a future project manifest (not yet designed) may redefine root as its own location, but that's an open question for later, not a v0.0.4 concern.
- Path segments `.`, `..`, `/`, `\` are rejected.
- Circular imports are a diagnostic, not a silent stack overflow.
- `public fn` and `public type` are visible through the module alias; plain `fn`/`type` stay module-private. Without `public type`, a struct could never cross a module boundary at all — a module could expose a constructor function but callers would have no way to name or read fields of the type it returns. Both keywords behave identically: `[ 'public' ] 'fn' ...` and `[ 'public' ] 'type' ...`.
- Imports never inject into global scope — always namespace-qualified. **No exceptions, including stdlib.** `println` is always `io.println(...)`; there is no globally-injected builtin form. (This is a deliberate reversal of the v0.4.5 reference implementation, which called `println(msg)` unqualified — that form is not carried forward.)
- **v0.0.4's own tests don't need the stdlib.** Module resolution, qualified calls, `public` visibility, and circular-import detection are all fully exercisable with user-defined modules alone (e.g. a local `support/math.kai` with `public fn add(a: int32, b: int32) -> int32`). The stdlib itself is deferred (originally to v0.0.5, re-anchored to the `kai.toml` manifest design by v0.26, §3.7) — implementing any of it now against types that don't exist yet would just be thrown-away work.

### 3.7 Standard library (built-in, no disk resolution)

**Version note:** initially deferred to v0.0.5 — every stdlib signature here depends on `string` and/or arrays, neither of which exist before v0.0.5 (Ownership runtime, §7), so implementing any of it earlier would have been discarded once those types landed. That deferral was then re-anchored (whitepaper v0.26) from a fixed version to the `kai.toml` project-manifest design: every function here is surface API over already-existing types, so it introduces no core semantic rule of its own and is not a gate on §5's *test* builds — see the boundary line below.

| Import | Surface |
|---|---|
| `std.io` | `println`, `print`, `eprintln`, `readln` |
| `std.fs` | `exists`, `read_to_string`, `write`, `append`, `remove`, `rename` |
| `std.env` | `get`, `cwd` |
| `std.str` | `parse_int`, `parse_float`, `join` |
| `std.math` | `sqrt`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `pow`, `abs`, `min`, `max` |
| `std.time` | `now`, `millis`, `sleep_ms` |

Everything above this line — **and every line of §3's semantics** (types, ownership, modules, control flow) — must be fully specified, implemented, and tested before Section 5 begins. This is the scope boundary for v0.0.1–v0.0.6 (the new v0.0.5 ownership-runtime slot plus what was v0.0.5 and is now v0.0.6).

**Amended by v0.26.** The sentence above was historically read as a hard, absolute gate on the *entirety* of §3.7's stdlib surface, and was crossed implicitly when v0.0.7–v0.0.9 shipped before the stdlib ever landed (the deferral-to-manifest was only recorded as a v0.0.6.1 changelog note, never here). This amendment makes explicit what was in fact already true and safe:

- The **gate's intent** — that §3's *semantics* be complete and tested before §5 — was satisfied and remains binding. The stdlib is not part of those semantics: §3.7 adds only API functions over types that already exist, and introduces no new type-system, ownership, or codegen rule that §5 could unknowingly depend on. No §5 test has ever needed the stdlib, so the deferral carried no testing risk.
- The **stdlib's actual requirement** is re-anchored: it is mandatory before any §5 feature (`dsl sql`, `dsl api`, `kai sync`) is exercised by a **real, non-fixture Kai program** — concretely landed with, or before, the `kai.toml` project-manifest design that also scopes `dsl` snapshots and §5.6's per-project `kai debt` severity overrides. In test/fixture-only form, §5 features continue to not require it.

---

## 4. Non-goals (explicit, to prevent scope creep)

- Kai is not a general distributed-systems protocol. (See prior exploration of NEBULA/pub-sub — out of scope for the language itself; could be a future std module at most.)
- Kai is not trying to replace Rust/Go for systems programming, embedded, or performance-critical compute. Its target is backend services with heavy external integration surface.
- The effect system (§5) is not a general-purpose algebraic effects system à la Koka. It covers exactly the instances of the Trust taxonomy defined in §5.0 (Contract, Temporal, Correctness, Reversibility) plus Signal (§5.2, non-Trust telemetry). New effect or Trust kinds require a written amendment to this document, not an ad-hoc addition during implementation.
- `dsl sql` / `dsl api` are not a full ORM or query builder. They validate hand-written queries against a schema/spec snapshot; they do not generate queries.

---

## 5. Trust-aware layer (v0.0.7+ scope — built only after Section 3's semantics are complete and tested; the §3.7 stdlib surface is re-anchored to the `kai.toml` manifest design per v0.26, not a hard gate on this section's test builds)

This is opt-in. None of it changes how a function looks unless the function actually depends on external trust.

### 5.0 What "Trust" formally means

Kai's position is not "a language with several safety features." It is: **external trust is a thing to be modeled, and every trust-aware feature is an instance of one abstract structure, not a coincidence of four features living in the same compiler.**

```
Trust⟨C⟩ = (Claim, Origin, Verification, Decay, Violation)
```

- **Claim** — a proposition about `C` that the code depends on, which the compiler cannot guarantee from the code alone.
- **Origin** — the authority the claim rests on: a schema snapshot, the wall clock, an accumulated history of observation, a compiler-captured pre-mutation Place snapshot.
- **Verification** — when/how the claim is checked: statically (compile-time, against a snapshot of Origin) and/or dynamically (runtime, against a live Origin).
- **Decay** — the mechanism by which a claim that was once true can become false **without the depending code itself changing** (a vendor changes a spec, time passes, state mutates, an external system goes down).
- **Violation** — the consequence when Verification finds Claim false. This is **mandatory, not optional**, and always one of exactly two kinds: hard failure (panic, terminal) or a defined non-terminating signal. Never silent.

**A corollary that resolves an open question from the original draft:** if something has no defined Violation, it is not a Trust — it is mere information. This is why `assume`, which tried to be both a hard invariant and a soft statistic at once, is retired in favor of two separate, formally distinct constructs (§5.2).

**`kai debt` is not a fifth feature — it is a projection of Trust state.** Nothing about the debt ledger is independently designed; it falls directly out of the five-field structure above:

```
Trust⟨C⟩ = (Claim, Origin, Verification, Decay, Violation)
                              ↓
                        Trust state
                              ↓
                          kai debt
```

Concretely: **Debt = unresolved Trust violations or degradation** — an entry exists in `kai debt` only when some Trust's Verification has found (or is at risk of finding) its Claim false. `observe` produces no debt because it produces no Trust at all (§5.2) — it is Signal, shown separately in `kai debt` output as informational history, never counted as debt. This boundary is deliberate and load-bearing: without it, the ledger drifts toward "a dashboard of everything we ever observed," which defeats its purpose as an actionable, prioritizable list.

**Syntax is an ergonomic surface over one semantic IR, not four (or five) independent forms the effect checker must special-case.** `require user.age > 0;`, `Token @wallclock(30m)`, and `dsl api("stripe", v3) -> PaymentIntent { ... }` are different syntax, but the effect checker (§8) does not reason about them as "a require statement" vs. "a duration type" vs. "a dsl block." It lowers each into a `Trust<C>` value — Claim, Origin, Verification, Decay, Violation, all filled in — and everything downstream (drift detection, `kai debt`, override handling) operates uniformly over `Trust<C>`, never over the originating syntax form. A new Trust-producing syntax form only needs a lowering rule into this IR; it does not need new handling threaded through every downstream phase. See §8, constraint 8, for the compiler-side requirement this implies.

The four Trust instances, and the one non-Trust Signal, as one table:

| Kind | Claim | Origin | Decay | Violation |
|---|---|---|---|---|
| Contract (§5.4) | external shape matches TAST | build snapshot / live schema | vendor changes spec | build warning / runtime panic + debt (§10.6) |
| Temporal (§5.1) | value still within its valid window | logical flow position or wall clock | time passes | compile error (unhandled expiry) / panic |
| Correctness (§5.2, `require`) | a program invariant holds | programmer declaration | state changes | panic, always, no exception |
| Reversibility — Transactional (§5.3) | the mutation's pre-mutation Place value is captured before the write and can be restored exactly during unwind | compiler-captured pre-mutation snapshot of the Place | — | rollback failure is a distinct trap |
| Reversibility — Compensatable (§5.3) | mutation has an explicit compensating action (`state_after + compensator ≈ acceptable_new_state`, not a true inverse) | declared `compensate` block | — | compensation failure is a distinct trap |
| *(Signal, not Trust)* `observe` | none — pure observation | history of observed values | — | none; has no Violation, so it does not qualify as Trust at all |

`reversible` remains the single user-facing keyword — a function marked `reversible` may contain both Transactional and Compensatable effects — but the compiler internally treats them as two distinct Trust kinds with two distinct guarantees, precisely so the keyword never promises a stronger guarantee (exact restoration from a pre-mutation snapshot) than what a given effect can actually provide (best-effort compensation). This is why "rollback" and "compensation failed" are reported as different trap messages in §10.4, not a single generic "reversal failed."

Compiler, runtime, and `kai debt` are all, structurally, just different points in the same lifecycle: **produce → consume → invalidate → verify** a Trust. A new feature only belongs in Kai if it can be written into this table with all five fields filled in (or explicitly marked as a Signal, if it has no Violation).

### 5.1 Temporal validity — `@local` and `@wallclock`

A single `@duration` modifier conflates two different Origins, which is a real semantic hole: "flow-relative time" only makes sense for code the compiler can trace through a linear control flow. It says nothing once a value crosses a boundary the compiler can't see through — `queue.send(token)` consumed by a worker 40 minutes later, a value that gets serialized, or handed across threads. Kai splits the modifier into two, matching two different Origins:

```kai
Token @local(30m)       // Origin = compiler-traced flow position — checked statically, cheap, no runtime cost
Token @wallclock(30m)   // Origin = an actual embedded timestamp — checked at runtime, re-verified every use
```

```kai
fn refreshSession(session: Token @local(30m)) -> Result<Token @local(30m), Error> {
    ...
}

fn useSession(session: Token @local(30m)) -> unit {
    // compiler error if this call happens after 30m of flow-relative time
    // without a refresh() or expired-branch in between
}
```

#### 5.1.1 The boundary, defined mechanically — not by keyword list

> A **compiler-untraceable boundary** is any operation after which the compiler cannot statically prove that the continuation executes within the same local temporal context as the originating computation.

This is expressed as an **effect**, `escapes-local-context`, on the *operation*, not as a hardcoded list of "async-sounding" constructs — a queue happening to use a worker thread internally doesn't matter; what matters is whether the compiler can still prove context continuity through it.

| Construct | Boundary? |
|---|---|
| Ordinary function call | No — `@local` unaffected |
| Inline/block expression | No |
| `spawn` a task | Yes |
| Queue send / enqueue | Yes |
| Channel send consumable by another task | Yes |
| Thread-pool hand-off | Yes |
| IPC / network send | Yes |
| Persistent storage | Yes |
| Synchronous channel with a compiler-proven same-context consumer | No |
| `await` | Depends — boundary only if the continuation's context isn't provably preserved |

```
Γ ⊢ e : T @local
Γ ⊢ op : escapes-local-context
────────────────────────────────
Γ ⊢ op(e) : T @wallclock
```

New primitives that cause an escape are handled by giving them the `escapes-local-context` effect — the boundary *definition* never needs to change, only the set of things tagged with it. This is the same design instinct as `Trust<C>` itself (§5.0): one abstract mechanism, extensible by tagging new instances, not by amending the mechanism's own definition each time.

#### 5.1.2 Effect inference — transitive over the call graph, attached to the function

The `escapes-local-context` effect is not just a property of primitives — it propagates transitively through every function (and closure, §5.1.3) that calls something carrying it, directly or indirectly:

```
effect(f) = direct_effects(f) ∪ ⋃ effect(g), for every g called by f
```

Without this, abstraction silently breaks the boundary rule: a wrapper function that merely forwards a `@local` value into `queue.send` would otherwise look "safe" to any caller, because the effect would appear to stop at the primitive. Cycles in the call graph (mutual/direct recursion) are resolved as a least-fixed-point problem over the graph's strongly-connected components — a `Set<Effect>` per function from the start, not a bare boolean, since §5's roadmap (`io`, `blocking`, and others) will add more effect kinds later and a boolean would need to be redesigned the moment it does.

**The effect is a property of the function's signature, never inferred from a parameter's type.** A function can escape without returning anything (`fn log(t: Token @local(30m)) { queue.send(t); }`), and can eventually carry more than one effect simultaneously — trying to infer escaping-ness from a return type would be both incomplete and the wrong place to attach it:

```
enqueue_job : (Token @local(30m)) → Unit ! escapes-local-context
```

**Declared effect annotations are a verified contract, not a trusted assertion — and this makes them a `Trust<C>` instance in their own right (§5.0):**

```kai
fn enqueue_job(t: Token @local(30m)) effects { escapes-local-context } {
    queue.send(t);
}
```

| Trust field | Effect contract |
|---|---|
| Claim | this function's actual effects are a subset of its declared `effects { ... }` set |
| Origin | the `effects { ... }` annotation the programmer wrote |
| Verification | compile-time, `inferred_effects(f) ⊆ declared_effects(f)` |
| Decay | the function body changes (e.g. a wrapper that didn't escape starts calling `queue.send`) while the annotation is left stale |
| Violation | compile error — `inferred ⊄ declared` |

A declaration may be more conservative than what's strictly inferred (declaring `escapes-local-context` on a function whose body doesn't currently need it is allowed — the language permits over-declaring), but never less: `fn f(...) effects {} { queue.send(...); }` is a compile error, not silent trust in the annotation. This is exactly why the annotation exists as an *addition* to inference rather than a replacement for it — annotations matter most at public-API boundaries, where a caller depends on the effect set staying stable even as the implementation changes underneath it; without verification, that stability claim would be exactly the kind of unconditionally-trusted assumption §5.0 exists to rule out.

That this effect-contract mechanism independently reduces to the same five-field `Trust<C>` shape, having been designed from scratch for a completely different concern (temporal boundaries, not schemas or invariants), is a real validation of §5.0's abstraction — not a coincidence engineered after the fact.

#### 5.1.3 Closures are first-class nodes in the effect/call graph — no second boundary rule

A closure is analyzed exactly like any other function value — it has a body, it can be assigned an effect set, and it participates in the same call-graph inference as `fn` declarations. No separate "closure boundary rule" exists; treating closures as ordinary graph nodes is the direct consequence of the model already established, not an exception to it.

What closures add is **capture provenance**: a closure's summary must track not just its effects but which `@local` values it captured, since a `@local` value can escape through an environment without ever appearing as a direct call argument:

```kai
fn schedule(t: Token @local(30m)) {
    let job = fn() -> unit { queue.send(t); };   // t captured, not passed as an argument
    dispatcher.run(job);
}
```

```
ClosureSummary {
    effects:  Set<Effect>
    captures: Set<(VarId, Type)>
}

job: effects = { escapes-local-context }, captures = { t: Token @local(30m) }
```

The boundary rule generalizes accordingly — it's about *reachability*, not just direct arguments:

> **No `@local` value may become reachable from a value whose execution may cross an `escapes-local-context` boundary without first being converted to `@wallclock`.**

This single invariant covers direct arguments and closure captures uniformly — it's strictly stronger and more general than an "arguments only" version would be, and is the form the rule takes going forward.

**Function values that are returned or stored carry their summary with them, as part of the value's type — this is flow-sensitive tracking of a summary, not arbitrary higher-order flow analysis:**

```kai
fn make_job(t: Token @local(30m)) -> Job {
    return fn() { queue.send(t); };   // Job's type now carries effects={escapes-local-context}, captures={t: @local}
}

let job = make_job(t);
store.push(job);   // store.push is a boundary; job's carried summary is what's checked
```

**Scope explicitly bounded for v0.0.7** — this is deliberately not a general points-to/escape analysis:

- Direct call → call-graph edge.
- Statically known closure invocation → closure node, analyzed like any function.
- Closure passed as an argument → its effect/capture summary travels with the argument.
- Closure returned or stored → its summary travels with the value's type.
- Fully dynamic dispatch (closure selected at runtime, target unknowable statically) → conservative union over all possible targets:
  ```
  effects(f)  = ⋃ effects(target)  for every possible target
  captures(f) = ⋃ captures(target) for every possible target
  ```
  This can produce false positives (rejecting a program that would, at runtime, only ever hit a non-escaping branch) but never a false negative — consistent with §3.3's cyclic-struct precedent and §9.10's closure-cycle rule: conservative-and-decidable beats sound-but-expensive or unsound-but-cheap.

#### 5.1.3a Local-read narrowing — `T @local(d)` flows to a plain-`T` argument position; propagation is gated by the callee's effect, not by parameter syntax

An early version of this section's boundary check had a real usability gap, found empirically once the boundary-rejection tests were run against ordinary, non-escaping code: passing a `@local`-typed value to a parameter typed as plain `T` (no modifier) was rejected outright, at typecheck, via exact-type matching — regardless of whether the callee actually escaped anything. That's wrong: since `@local` is zero-runtime-footprint, pure delegation (§5.1.7), a function that doesn't care about temporal validity at all should be usable with a `@local`-tracked value without friction. Rejecting it unconditionally would have made `@local` "contagious" through any call graph wider than the narrowest examples — every function touching a `@local` value anywhere, escaping or not, would need to spell out the same modifier, contradicting the "cheap, zero cost" positioning this section opened with.

**The fix, and why it belongs at the right phase:** `T @local(d)` is allowed to be *read* as plain `T` at an argument position (direct calls and closure calls alike) — this is a marker-drop on the read, not a representation-level conversion (there's nothing to convert; §5.1.7 already established `@local` has no runtime representation of its own). Critically, the modifier is **not stripped from the TAST node** — the value's actual tracked type is preserved. What changes is where the safety decision gets made:

- **Typecheck cannot decide this.** Per §8's locked phase order (typecheck → ownership resolution → effect-check → codegen), the callee's effect set isn't known yet at typecheck time — a function's `escapes-local-context` status may itself be transitively inferred (§5.1.2), which requires the call graph, not just the immediate signature. Typecheck accepting the argument shape and deferring the actual soundness question is the only phase-consistent option; rejecting or accepting outright at typecheck would mean guessing at information that phase doesn't have.
- **The effect checker makes the real call**, once it has the callee's effect set (declared or transitively inferred):
  - Callee has no `escapes-local-context` effect → passes silently. `log_token_id(t: string)` (an ordinary function that never escapes) is fully usable with a `@local`-tracked argument, exactly as it should be.
  - Callee has `escapes-local-context` → the boundary rule (§5.1.1–§5.1.3) applies and rejects it, now via the *correct* mechanism (a genuine boundary-crossing diagnostic) rather than an incidental type mismatch.

This keeps the invariant from §5.1.3 exactly as stated — *no `@local` value may become reachable from a value whose execution may cross an `escapes-local-context` boundary without first being converted to `@wallclock`* — while no longer over-applying it to code that was never at risk. Contagion is bounded by what a value's reachable call graph actually does, not by how literally a parameter's type was spelled.



#### 5.1.4 `@wallclock` → `@local`: no conversion path in v0.0.7

Not "implicit is disallowed, explicit is available" — there is **no conversion mechanism at all** in this direction for v0.0.7. A `@wallclock` value has already lost the local execution-context provenance that would be needed to reconstruct a `@local` claim; adding an explicit-but-legal conversion operator is a separate, larger design question (does it ever make sense? under what conditions?) that this version doesn't need to answer. `@local → @wallclock` at a boundary crossing remains the only direction that exists.

#### 5.1.5 `@wallclock` wire format

Conservative and canonical for v0.0.7: **RFC 3339 / ISO-8601, UTC only, fixed microsecond precision, `Z` suffix mandatory** — no local offsets.

```
2026-08-24T19:42:31.123456Z   ✅
2026-08-25T02:42:31+07:00     ❌ (local offset — rejected)
```

Contract:
```
serialize(t)          → canonical UTF-8 RFC 3339 UTC string, microsecond precision
deserialize(serialize(t)) == t     (for every t representable at this precision)
```

Two producers representing the same instant always serialize to the identical byte string — canonical form makes equality testable as string equality, not a timestamp-aware comparison. Microsecond precision is a deliberate ceiling for v0.0.7 (sufficient for practical event ordering without an absurdly long nanosecond representation); if nanosecond precision is ever needed, that's a versioned wire-format change, not an ambiguity baked into v0.0.7's format.

#### 5.1.6 `DurationLit`

```
DurationLit  ::= DecimalInt DurationUnit
DurationUnit ::= 'ms' | 's' | 'm' | 'h' | 'd'
```

Integer-only, deliberately, for v0.0.7 — `30m`, `1h`, `500ms`, `2s`, `7d` are valid; `1.5h`, `1hour`, `-30m`, and a bare `30` with no unit are not. Fractional durations are a grammar extension to add explicitly later if needed, not a lexical ambiguity to leave lurking now. **Lexical validity and semantic validity are deliberately separate**: `0ms` parses fine (the grammar has no opinion on it) — whether `@local(0ms)` is a legal *duration for a temporal type* is a typecheck/semantic question, not a lexer concern.

#### 5.1.7 Heap-bearing-ness and ownership for `@local` vs. `@wallclock` — asymmetric by design

The two Origins (§5.1) are not just two branches of one uniform mechanism — they have genuinely different runtime footprints, and conflating them was a real, found bug (a leak in `@local` release, traced to codegen treating `Temporal` as a no-op instead of delegating). The corrected rule, precise enough to implement directly:

> **`@wallclock` is unconditionally heap-bearing, regardless of its inner type — it carries a runtime-allocated header (the embedded instant) that must exist no matter what.** **`@local` has zero runtime footprint of its own and delegates entirely to its inner type's own heap-bearing-ness** — it's a compile-time-only position marker (§5.1, "checked statically, cheap, no runtime cost"), not a wrapper.

This mirrors two *different* precedents already established, one per Origin, not one precedent stretched to cover both:
- `@wallclock`'s unconditional-heap rule is the same shape as **array** (§9.1) — the container needs storage regardless of payload.
- `@local`'s pure delegation is *more minimal* than `Optional`/`Result`'s conditional delegation (§9.9a) — those still wrap a tag; `@local` doesn't even do that, it adds nothing at compile time beyond a type-level marker, and nothing at runtime at all.

Concretely:

| Type | Heap-bearing? | Why |
|---|---|---|
| `int32 @local(30m)` | No | Pure delegation — `int32` is stack, `@local` adds nothing |
| `int32 @wallclock(30m)` | **Yes** | Unconditional — needs a header for the embedded instant even though `int32` itself is stack |
| `string @local(30m)` | Yes | Delegation — `string` is heap, `@local` doesn't change that |
| `string @wallclock(30m)` | Yes | Unconditional heap header **plus** a heap-bearing inner — see the two-step release below |

**Runtime representation.** A `@wallclock`-typed value is a header, not a raw inner value:
```
KaiWallclock {
    rc: i64
    instant: i64      // UTC microseconds since Unix epoch — NOT an RFC 3339 string
    payload: <inner's representation>
}
```
The header stores a compact integer instant, never the RFC 3339 string form. §5.1.5's wire format is a *serialization contract* — what a `@wallclock` value looks like crossing a boundary (network, disk, queue) — not a claim about in-memory representation. Storing and repeatedly comparing an RFC 3339 string against the clock would be real, needless overhead for something checked "at every use" (§5.1); conversion to/from the canonical string happens only at the serialize/deserialize boundary itself. (The comparison operation this instant enables — checking a `@wallclock` value against the current clock — is a separate, still-open codegen question, tracked alongside the rest of §5.1's runtime-check machinery; this section only locks the header's storage shape, which the release cascade below depends on.)

**Release is two-step whenever the inner type is also heap-bearing** — release the header itself, but first cascade into the payload, exactly like an array releasing its elements before freeing its own buffer (§9.9):

```
release(v: T @wallclock(d)):
    if heap_bearing(T):
        release(v.payload)     // cascade first
    release_header(v)          // then the wallclock header itself (rc-based)

release(v: T @local(d)):
    release(v)                 // pure delegation — release the inner value directly,
                                // there is no header to release
```

Skipping the cascade step for a heap-bearing inner (releasing only the header) is exactly the kind of leak this section exists to rule out by being explicit — a bare `call_void(release_header)` with no conditional cascade is *silently correct* for `int32 @wallclock` and *silently wrong* for `string @wallclock`, which is precisely the failure mode that made this worth locking down rather than leaving to per-callsite judgment.

This asymmetry applies uniformly everywhere heap-bearing-ness is queried — codegen's own `heap_bearing` check, closure environment dtors (§9.10, closures capturing a `@wallclock` value follow the array-precedent unconditional-heap branch, not `Optional`'s conditional one), and retain insertion — all of it, not just the release path this section was prompted by.

**Equality is inner-value equality — the instant never participates in `==`.** For two Temporal values with identical origin, duration, and inner type, `==`/`!=` compare the *payload* only (for `@wallclock`, extracted from each header; for `@local`, the bare inner values directly). This was found unspecified during v0.0.8.4 stabilization when temporal `==` was first compiled, and is locked here as a semantic decision rather than an implementation side effect. The reasoning: folding the instant into `==` would define equality as issuance-time identity, destroying `==`'s fundamental property as a pure function of value contents — `t == t` could flip to false after one copy crosses expiry, or when two content-identical tokens were created microseconds apart. Validity status is orthogonal to value identity: an expired token and a fresh token with equal text are *equal values* in different validity states — and validity is exactly what the verification machinery (`Trust<C>`, §5.0/§5.1) governs, never `==`. The precedent already exists: string `==` compares content, never allocation identity (§9.7). If a future need arises for claim-level identity (same text *and* same issuance time), it must surface as a separate accessor or verification-API check — never as `==` overloading. The typecheck gate remains strict: both operands must be `Temporal` with identical origin, duration, and inner type; mixing temporal with plain, or mismatching any component, is rejected at typecheck.

### 5.2 Correctness and observation — `require` and `observe`

The original single `assume` construct tried to be two things that turned out to have incompatible semantics: a hard invariant (violating it is always fatal, so a confidence score attached to it changes nothing about program behavior) and a statistical observation (which, if it's ever allowed to be fatal, isn't really "statistical" at all). Kai splits these into two constructs — only one of which is a Trust.

**Scoping decision, stated up front: neither construct needs the call-graph *inference* machinery from §5.1 (`EffectName`/`EffectSet`, SCC/fixpoint propagation, `effects { ... }` contracts).** That machinery exists because a temporal boundary crossing is a *static* soundness property — a caller must know a callee's effect set, possibly transitively inferred, before it can decide whether `@local` is still legal at that call site (§5.1.2, §5.1.3a). `require`/`observe` have no analogous cross-function static concern: a violation happens at the exact point the statement executes, and no decision a *caller* makes depends on it — nothing needs propagating. Building the inference apparatus here would be machinery with no problem to solve — consistent with §4's non-goals discipline.

**This does not mean `require`/`observe` bypass `kai-effects` entirely — only its inference subsystem.** They still lower into `Trust<C>` (§5.0) through the same crate, exactly as originally planned for it (§8: one crate, one `Trust<C>` consumer, new lowering rules rather than new crates per feature) — `require` lowers to a Correctness Trust value, `observe` to a Signal. This lowering is local and immediate (the TAST statement maps directly to one `Trust<C>` value, no graph to traverse), which is precisely *why* no inference layer is needed for it — but it still has to happen, because `kai debt` (§5.6, "a projection of Trust state") and `@override` (§5.5) both operate on `Trust<C>` uniformly, and `require`'s debt-ledger entry (§10.3) has nowhere to come from if it skips this lowering step.

#### 5.2.1 `require` — Correctness Trust

```kai
fn getDiscount(user: User) -> Percent {
    require user.age > 0;
    ...
}
```

**Trust<C> lowering**, filling in §5.0's table with the mechanics specific to `require`:

| Field | Value |
|---|---|
| Claim | the asserted boolean expression holds |
| Origin | the programmer's `require` declaration at this exact point in control flow |
| Verification | runtime, synchronous, at the statement itself |
| Decay | stateful — the invariant can become false as program state changes, without the `require` statement's own code changing |
| Violation | panic, unconditional (§10.3) — no confidence score, no soft form |

**Evaluation is synchronous and exactly-once.** `require expr;` evaluates `expr` exactly one time, inline, at the point it appears in control flow — no laziness, no deferral, no reordering relative to surrounding statements. This is deliberately unremarkable: `expr`'s evaluation follows ordinary expression rules (borrows, temporaries released at end of statement per §9.11) with nothing `require`-specific about it — the only special behavior is what happens with the resulting boolean (§10.3's panic sequencing), not how the expression itself is evaluated.

**Condition text — used in the panic message (§10.1/§10.3) and in `observe`'s JSONL records (§5.2.2) alike — is the raw source-text span of the expression, never an AST pretty-print.** It's a direct slice of the original source, using the span already carried by `Diagnostic`/TAST nodes (§8) — no new pretty-printer, and no risk of the rendered text drifting from what the programmer actually wrote (which a reconstruction could introduce via whitespace/parenthesization normalization). If the expression happens to span multiple lines, it's emitted verbatim: JSON string encoding already escapes embedded newlines without breaking the one-record-per-line JSONL format, and the plain-text panic message simply includes them as written — no special formatting rule invented for what's a rare case in practice.

Full runtime failure sequencing (record to debt ledger, then panic) is specified in §10.3 and is not repeated here — this section owns the Trust<C> shape and evaluation semantics, §10 owns the failure path.

#### 5.2.2 `observe` — Signal, not Trust

```kai
fn getDiscount(user: User) -> Percent {
    observe user.age > 0;   // logged, never fatal, purely observational
    ...
}
```

Pure telemetry: records how often a condition holds, purely for visibility, and never affects control flow or panics. Because it has no Violation, it does not fit the Trust structure (§5.0) at all — it is not tracked in `kai debt` as debt (nothing is owed), only surfaced as informational history (§5.6/§9.9a already established this split; this section defines what actually gets recorded and where).

**What gets recorded, per evaluation:** `{ timestamp, source location, condition text, boolean outcome }`. Like `require`, evaluation is synchronous and exactly-once — no batching, no deferred recording.

**The sink — resolved for v0.0.8, deliberately minimal.** Per §2.1 (static and offline by default — nothing reaches outside the project without an explicit, separate step), the default sink is a **local, append-only file**, not network telemetry:

```
.kai/observe.log   — JSONL, one record per line:
{"timestamp": "...", "location": "src/billing.kai:12:5", "condition": "user.age > 0", "outcome": true}
```

The sink is a pluggable interface internally (so a future version can add alternative sinks without redesigning `observe` itself), but v0.0.8 ships exactly one implementation — the local file above — and no opt-in telemetry or network sink of any kind. **This is explicitly not full `kai debt` integration.** Aggregating these raw records into `kai debt`'s dashboard view (§5.6) is v0.0.12 scope, already scheduled after `require`/`observe` (v0.0.8) and both `dsl` layers (v0.0.10–11) on the roadmap (§7) — v0.0.8's job is only to get the raw signal recorded reliably, not to build the ledger UI on top of it.

**Sink location without a project root — `compile(&str)`, the string API — is a documented no-op, never the invoking process's CWD.** `.kai/observe.log` and `.kai/debt.log` (§10.3) are project-relative paths, and `compile(&str)` (v0.0.4, scoped explicitly to sources without `use` — not a full-project API) has no project root to be relative to. Falling back to CWD here would directly contradict §3.6's project-root rule for module resolution, which deliberately rejected CWD for the same reason: it's external environmental state that makes behavior depend on where a command happens to be invoked from, rather than on the input itself. Rather than reintroduce that non-determinism through the back door, the sink is a documented no-op when compiling via the string API — recording is skipped, explicitly and predictably, not written to some incidental directory the caller didn't choose. This is a deliberate limitation of `compile(&str)`, not a silent gap: it's written down here precisely so it isn't rediscovered as a surprise later. (A future version could let callers of the string API supply an in-memory sink explicitly, avoiding the filesystem question entirely — out of scope for v0.0.8.)

If a `require` invariant should instead be soft-tracked for now while a fix is planned, that is what `@override` (§5.5) with `kind: "suppress"` and a mandatory expiry is for — not a weaker version of `require` itself, and not `observe` either (which never carries a Violation, so it cannot express "I know this might be wrong for now").

### 5.3 Reversibility — `reversible`, and why "rollback" isn't always the right word

A function marked `reversible` must have every mutation inside it be either automatically restorable from a captured pre-mutation snapshot or explicitly paired with a manual reversal — but these are not the same guarantee, and conflating them was a real gap in the original draft. Database-style rollback is atomic and guaranteed once it runs, restoring the exact prior state from a snapshot. A compensating action against an external side effect (an email already sent, a webhook already fired) is not — it is a best-effort action that can itself fail, and it only reaches `state_after + compensator ≈ acceptable_new_state`, an approximation, not a true inverse. Kai keeps these as two distinct Reversibility subtypes at the semantic level (§5.0's table: Transactional vs. Compensatable) and reserves "rollback" for the first one only.

```kai
fn transferMoney(from: Account, to: Account, amt: Money) reversible {
    from.balance += amt;   // transactional — pre-mutation Place snapshot captured
    to.balance += amt;     //   same: snapshot of to.balance taken before the write
}
// On panic: the runtime restores both snapshots in reverse order (§10.4).
// On normal return: the snapshots are released and the mutations commit.
```

```kai
fn onboardUser(user: User, fee: Money) reversible {
    user.status = "onboarded";                        // transactional — pre-mutation Place snapshot

    sendEmail(user.email) compensate {                // a call: compensating — never automatic, must be declared
        sendEmail(user.email, "disregard previous email")
    }

    chargeCard(user, fee) compensate {
        refundCard(user, fee)                         // still just a best-effort action, not a guarantee
    }
}
```

The compiler never generates a compensating action automatically — unlike a Place snapshot, there is no general rule for "the opposite of sending an email," so it must always be authored by hand. Anything with neither an automatically-restorable Place mutation nor a declared `compensate` block is a compile error, not a silent gap (unchanged from the original rule — only the vocabulary around external effects has changed).

If a `reversible` function panics partway through, the mutations already applied are not left dangling — see §10.4 for the mandatory unwind behavior, which now distinguishes rollback (transactional) from compensation (external, best-effort, and itself fallible).

#### 5.3.1 Pre-Mutation Place Snapshot

**Scope decision (signed off): all assignments whose destination is a writable Place are transactionally reversible, subject to the Place's ownership and storage semantics.** This is a deliberate widening from §7's original wording ("automatic invertibility for arithmetic mutations"). The Place model (§9.3) exists precisely to unify plain bindings, struct fields, and array elements under one rule — restricting snapshot tracking to arithmetic operators (`+=`/`-=`/`*=`/`/=`/`%=`) would reintroduce the per-operator special cases §9.3 was built to eliminate ("one rule, not a struct-field rule and a separate array-index rule"). A snapshot taken from a Place does not care whether the destination was reached via `x +=`, `x =`, `arr[i]`, or `p.a.b =`. Arithmetic was the illustrative case in the roadmap, not the actual boundary.

**Pre-Mutation Place Snapshot** is the formal term for the Transactional mechanism: the value a Place held immediately before the compiler performs a write to it, captured and retained for possible restoration during unwind. A transactional snapshot captures the *semantic value* of the Place, not an arbitrary raw memory image. For stack-only values (`int32`, `float64`, `bool`, `unit`) this is equivalent to a bitwise copy. For heap-bearing values (`string`, `array`, `struct`-with-heap-field, `closure`) the snapshot participates in the ownership model (§9): it holds an independent retain on the prior value, sufficient to restore it safely during unwind. It is therefore an owning reference with its own lifetime — created at the mutation site, released on commit (normal return, the value was never restored), or consumed during unwind (the value is written back to the Place and the displaced current value is released).

Mutating a heap-bearing Place:

```
mutate(place, new_value):                          // inside a reversible fn
    old = read(place)
    if heap_bearing(type_of(place)):
        retain(old)                                 // snapshot holds an owning ref
    ledger.push(Transactional { place, snapshot: old })
    store(place, new_value)                          // ordinary §9.4 write

// commit path (normal return):
    for entry in ledger:
        if heap_bearing(entry.place):
            release(entry.snapshot)                  // never restored, drop the retain

// unwind path (panic):
    for entry in ledger.reverse():
        current = read(entry.place)
        store(entry.place, entry.snapshot)           // restore prior value
        if heap_bearing:
            release(current)                         // release the displaced value
            // snapshot's retain is consumed by the store (the Place now holds it)
```

This ordering is the direct extension of §9.4's existing "prepare replacement before releasing old" rule (which already guards self-aliasing cases like `arr[0] = arr[0]`). Inside a reversible activation the old value gains a second potential consumer — the unwind path — so its release is deferred and routed into the ledger's ownership until either commit or unwind resolves it. Multiple mutations to the same Place within one activation push multiple entries, restored in reverse order on unwind — exactly what §10.4's "unwinds the accumulated effect history in reverse order" commits to; this section only makes precise what an entry is.

#### 5.3.2 Where the ledger nodes are emitted

LedgerPush is emitted during ownership resolution because it is a runtime object-management node attached to a mutation, analogous to the retain/release nodes already emitted by that phase (§9.5). The effect checker remains responsible only for validating whether the resulting transactional or compensatable effect is permitted — consistent with §8 constraint 8's uniform Trust<C> model, where ownership resolution produces the runtime nodes and codegen reads them mechanically (never inferring them ad hoc), and the effect checker validates the Trust semantics rather than emitting runtime structure.

### 5.3.6 Compensation Thunks and Capture Semantics

When a `reversible` function declares a `compensate` block for a call, the compiler generates an internal, runtime-only callback (a `CompensateThunk`) rather than a first-class `Closure` value. This ensures compensation logic is strictly bound to the activation ledger and cannot be leaked or invoked manually.

Because a panic unwind may execute the compensation long after the surrounding function has advanced, the thunk must capture its environment with precise semantics:

1. **Capture-by-value at registration:** Variables referenced inside the `compensate` block are captured by value (shallow copy) at the exact moment the `compensate` block is evaluated and registered to the ledger. Subsequent mutations to those variables in the outer scope do not affect the captured state.
2. **Ownership (Retain/Release):** Heap-bearing values captured by the environment are explicitly retained (incrementing their reference count) during registration. When the ledger is processed — whether via normal **commit** (thunk destroyed without execution) or **unwind** (thunk executed, then destroyed) — the captured values are released. This prevents memory leaks on success and use-after-free errors during rollback.
3. **Immutable Environment:** For simplicity and safety, bindings captured within a `compensate` block are strictly read-only (`snapshot immutable`). Attempting to mutate a captured variable inside a compensation block is a compile-time error. This avoids complex aliasing, nested ledgers for inside-thunk mutations, and ambiguities about which object is actually being mutated.
4. **Self-contained Storage:** The captured environment is fully self-contained. It never holds raw pointers to the caller's stack slots (`alloca`), ensuring the thunk remains safe to execute regardless of how the runtime traverses the call stack during an unwind.

### 5.4 External contracts — `dsl sql`, `dsl api`

Queries and external API calls are validated against a committed **snapshot**, not a live connection, at build time.

```kai
dsl sql -> UserWithOrders[] {
    select users.id, users.name, orders.total
    from users join orders on users.id = orders.user_id
    where orders.status = "PAID"
}
```

```kai
dsl api("stripe", v3) -> PaymentIntent {
    POST /payment_intents
    body: { amount: int, currency: string }
}
// warning: stripe v3 is 2 versions behind (v5)
// breaking change in v4: 'currency' now requires ISO 4217 enum
```

Snapshots are generated and refreshed only via `kai sync`, never implicitly during `kai build` (see §2.1, §6).

### 5.5 Overrides — escape hatch, always owned

```kai
@override(
    reason: "field 'amount' actually nullable, spec incorrect as of v3",
    kind: "corrective",              // "corrective" | "suppress"
    observed_spec_hash: "sha256:a3f9...",
    verified_by: "@yuda",
    verified: "manual-test",         // "manual-test" | "contract-test"
    date: "2026-07-15",
    expires: null                    // required (non-null) if kind = "suppress"
)
dsl api("stripe", v3) -> PaymentIntent {
    POST /payment_intents
}
```

- **corrective**: "the spec is wrong, here's the truth" — stays valid until the upstream spec itself changes again, at which point it's re-flagged for review.
- **suppress**: "I know this is breaking, I'm deliberately not fixing it yet" — must carry an `expires` date; the compiler upgrades it to a hard error once expired.

### 5.6 `kai debt` — the unified ledger

All Trust categories (Contract, Temporal, Correctness, Reversibility) report into one place. `observe` Signals are informational only and shown separately, since they carry no debt (§5.0, §5.2):

```
$ kai debt

Debt report:
  [contract]      sql(v12)               4 versions behind, since 2026-06-02 (80d)
  [contract]      stripe v3              breaking change in v4 unacknowledged
  [temporal]      Token@wallclock(30m)   expired 12x this week, unhandled in 3 call sites
  [correctness]   user.orders.len<10000  violated in production, 2026-08-19 — require, always fatal
  [reversibility] sendWelcomeEmail()     marked irreversible, no compensate block defined

Signals (informational, not debt):
  [observe]       user.age>0             held in 99.1% of 40,201 observed runs

Overrides: 2 (needs human review)
Drift:     5 contracts
```

`kai debt --ci` exits non-zero on unacknowledged HIGH-severity drift; MEDIUM/LOW do not block CI by default. Severity defaults are heuristic (e.g. a type change with no safe implicit cast is HIGH) and are overridable per-project via `kai.toml`.

---

## 6. Command lifecycle

| Command | Network/DB required? | Purpose |
|---|---|---|
| `kai new` | No | Scaffold project with an empty/dummy schema snapshot |
| `kai build` | No | Compile against committed snapshots only |
| `kai run` | No | Build + execute |
| `kai sync <source>` | Yes | Refresh schema/API snapshot from a live source |
| `kai debt` | No | Read the local debt ledger (populated at build/sync time) |
| `kai debt pay <id>` | Depends | Attempt to resolve a specific debt item (re-verify override, regenerate types, etc.) |

A project with zero `dsl` blocks never needs `kai sync` and never touches the network. This is non-negotiable per principle §2.1.

---

## 7. Versioned scope roadmap

Strict ordering. A version does not start until the previous one has a working, tested compiler phase for it — not just a parser that accepts the syntax.

| Version | Scope | Exit criteria |
|---|---|---|
| v0.0.1 | `fn main`, `return`, `int32` literal | Full lexer→parser→typecheck→TAST→codegen(LLVM) pipeline, as separate modules with the AST/TAST boundary enforced, on the smallest possible program |
| v0.0.2 | `let`/`var`, primitives, arithmetic, `if/else` | `let` vs `var` mutability enforced (§9.3) |
| v0.0.3 | `type` structs, struct literals, field read **and** field write (write gated by `mut`), function calls with params, `mut` parameters, cyclic-struct-definition rejection | Type checker does real signature/field matching; `Place` rule covers field access for assignment (grammar); `mut` on a stack-type parameter is local-copy-permission only, zero ABI difference, not observable by the caller (§9.3) — **no retain-rule claim here**, since v0.0.3 has zero heap-bearing types active; cyclic struct defs rejected via DFS over the `TypeDecl` graph with the cycle path in the diagnostic |
| v0.0.4 | `use` / module system, `public fn`/`public type` | Circular import detection tested; project root = entry-file directory (deterministic, not CWD); module resolution/qualified calls/visibility tested entirely via user-defined modules, no stdlib dependency |
| v0.0.5 | **Ownership runtime** — `string` (plain literals only, no interpolation), array literals + indexing, array element write, `for..in`, retain/release enforcement | Ownership-transfer retain rule (§9.5) actually exercised and tested for `return`/struct-literal/array-literal, now that heap-bearing types (`string`, arrays) exist to trigger it; `for..in` borrows each element per iteration (§9.9); `Place` generalized to array indexing (§9.3) — writability gated by root (`var`/`mut` param) uniformly with struct fields; replacement-into-owned-slot ordering (retain new before release old) tested against self-aliasing (`arr[0] = arr[0]`); the two-axis invariant (writability vs. mutation-visibility, §9.3) tested as an explicit matrix: `var`/`let` array roots, `mut`/plain array parameters, and a stack-only `mut` struct parameter side by side, confirming `arr[i]` visibility tracks the root's type category and never the element type; string `==`/`!=` tested as content comparison — same-content literals from different allocation paths (literal, constructed, retained, returned) all compare equal (§9.7); empty array literal without annotation rejected at typecheck; unrecognized string escape rejected at lex phase with a specific diagnostic |
| v0.0.6 | `Optional`, `Result`, closures, `_ = expr;` discard statement, `Ok`/`Err`/`Some`/`None` constructors | Tagged-union ownership (§9.9a) — active-payload-only, heap-bearing-only retain/release, one mechanism for both types; `T?`/`Optional<T>` parse to one canonical node, no desugar pass; `??` short-circuits (lazy RHS); `Ok`/`Err` construct `Result` with context-typing for the unconstrained type parameter (same mechanism as `None`/empty-array); `.unwrap_or()` works on both `Optional`/`Result` (no dedicated grammar production — resolved at typecheck via receiver type + field name), `catch` stays `Result`-only via the dedicated `CatchBlock` (the only trailing-expression block in the language); discarding `Optional`/`Result` as a bare statement is a diagnostic, `_ = expr;` is the sole escape hatch, `_` never valid as a normal binding/param/loop-variable name (§9.9b); closures unconditionally heap-bearing regardless of capture (§9.10); closure-cycle rejection enforced via closure-bearing-type poisoning over the existing `TypeDecl` DFS graph (§9.10, extends §3.3); no user-facing generic syntax anywhere — Optional/Result/closures are the only parametric machinery, and it's built-in |
| v0.0.7 | `@local`, `@wallclock`, `DurationLit`, `escapes-local-context` effect inference, `effects { ... }` contract annotation | Boundary table (§5.1.1) enforced; effect inference transitive over call/closure graph with SCC/fixpoint cycle handling (§5.1.2); declared-effect contract verified (`inferred ⊆ declared`), never trusted blindly; closures as first-class graph nodes with capture-provenance summaries (§5.1.3), covering direct call, statically-known closure invocation, closure-as-argument, closure-returned-or-stored, and conservative union-of-targets for dynamic dispatch; `@wallclock`→`@local` has no conversion path; wire format round-trips (`deserialize(serialize(t)) == t`); `@wallclock` unconditionally heap-bearing (header, compact integer instant — not RFC 3339 in memory), `@local` pure zero-footprint delegation to inner (§5.1.7), verified uniformly across `heap_bearing`, retain, release (two-step cascade when inner is heap-bearing, tested specifically for `T @wallclock` with heap-bearing `T`), and closure-environment dtors |
| v0.0.8 | `require`, `observe` | `require` violation always panics per §10.3, recorded to debt ledger before exit; `observe` never panics, tracked as Signal only, not debt |
| v0.0.9 | `reversible` (transactional + `compensate`) | Transactional mutation snapshots for writable Places; compiler-captured pre-mutation Place snapshots with ownership-safe restoration for heap-bearing values; mandatory reverse-order unwind on panic per §10.4, distinguishing rollback from compensation |
| v0.0.10 | `dsl sql` + snapshot mechanism | `kai sync` for at least one DB (e.g. Postgres) |
| v0.0.11 | `dsl api` + OpenAPI sync | |
| v0.0.12 | `@override` + `kai debt` unified ledger | |

**§3.7 stdlib re-anchor (v0.26):** none of the §5 rows above require the stdlib for their *test/fixture* exit criteria — that has been the actual record since v0.0.4, and this table's absence of any stdlib exit criterion is now accurate rather than an oversight. The stdlib becomes mandatory only before a §5 feature (`dsl sql`, `dsl api`, `kai sync`) is exercised by a real, non-fixture Kai program — landed with, or before, the `kai.toml` manifest design (see §3.7 and the v0.26 changelog entry). This does not add a new row; it reconciles this table with the amended §3.7.

Anything not on this table is out of scope until this document is amended.

---

## 8. Compiler implementation constraints

(Non-negotiable, independent of language design — this is what killed v0.4.x.)

Toolchain carried over from v0.4.5, kept deliberately: hand-written recursive-descent parser, inkwell/LLVM for codegen. These were never the problem. The problem was a missing boundary between "untyped AST" and "codegen input" — raw AST reached codegen directly, so when a real type system was retrofitted later, every codegen call site had to be revisited at once, mid-flight, with no way to land it incrementally. That failure mode is the single most important thing this section exists to prevent.

1. **Typed AST (TAST) is a distinct data type from AST, not the same struct with extra fields bolted on.** `ast::Expr` (parser output, untyped) and `tast::TypedExpr` (type-checker output, every node carries a resolved concrete type, every identifier resolved to an id not a string) live in separate modules and are never unified into one "flexible" enum.

2. **Codegen depends only on `tast/`, never on `ast/`.** This is enforced at the module-visibility level (`pub(crate)` boundaries), not left as a convention — codegen must be structurally unable to import raw AST. If codegen needs information it doesn't have, that information is missing from TAST and belongs in the type checker, not inferred ad hoc in codegen.

3. **Effect checking (`require`, `observe`, `reversible`, `@local`/`@wallclock`) runs after typecheck, before lowering** — it consumes TAST and produces a "checked" TAST (or rejects with a diagnostic). It never lives inside the type checker or inside codegen; it is its own phase with its own module, from the version it's introduced (v0.0.7+).

4. Lexer, parser, AST definitions, resolver, type checker, effect checker, and codegen are separate modules from commit #1. No file exceeds ~500 LOC without being split. AST/TAST node definitions contain no logic — only shape.

5. Every phase has its own unit tests, independent of end-to-end tests. TAST fixtures are asserted directly (no LLVM execution needed) separately from codegen fixtures (LLVM IR / execution output). Golden fixture files (`tests/fixtures/*.kai` + `*.expected`) back every language feature.

6. Diagnostics (`{ file, message, span, severity }`) are a first-class type from v0.0.1, not retrofitted. `file` is `Option<String>`/nullable in single-file phases (v0.0.1–v0.0.3, before modules exist) and populated from v0.0.4 onward once multiple files can be loaded into one compilation — this was surfaced as a real gap during v0.0.4 planning: `span` alone is ambiguous once more than one source file exists in the same diagnostic stream, and §10.1's panic format already implied a file-qualified location (`at src/orders.kai:42:9`) without the base `Diagnostic` shape actually carrying one.

7. LLVM/inkwell codegen is written from v0.0.1 onward (not deferred to an interpreter-first phase) — but only ever against TAST, never against raw AST, per constraint 2. If a version's scope has no type checker yet (there shouldn't be one without typecheck existing first — see §7), codegen for that version does not exist yet either.

8. **The effect checker operates over one uniform `Trust<C>` IR, not over per-syntax special cases.** Each Trust-producing syntax form (`require`, `@local`/`@wallclock`, `dsl sql`, `dsl api`, the `reversible`/`compensate` pair) has its own lowering rule from TAST into a `Trust<C>` value (Claim, Origin, Verification, Decay, Violation — §5.0). Everything downstream of that point — drift detection, `kai debt` population, override matching — reads only `Trust<C>`, never branches on which surface syntax produced it. Concretely: there is exactly one module that knows how to turn `require user.age > 0;` into a `Trust<C>`, and a separate, single module that knows what to do with any `Trust<C>` regardless of origin. Adding a new Trust-producing construct means writing a new lowering rule, not touching the debt/drift logic at all — this is the same "typed input, mechanical consumer" discipline as constraint 2, applied one layer up.

---

## 9. Ownership & memory model

**Framing note:** this section is a compiler implementation guarantee, not a developer-facing philosophy Kai users are expected to reason about day to day. `retain`/`release`/the co-owner mechanics below are never written by hand and never appear in Kai source — a Kai developer writes `let user = ...; foo(user);` and the compiler handles the rest. This section exists because §8 requires ownership resolution to be an explicit, mechanical IR-producing phase (so codegen never has to infer it) — it is documented at this level of detail for the compiler's sake, not because using Kai requires understanding reference counting. The only genuinely developer-facing surface from this section is §9.3 (mutability: `let`/`var`, `mut` parameters) — everything else here is what the compiler guarantees on the developer's behalf.

Kai uses compiler-managed reference counting for heap-bearing values. There is no tracing garbage collector. The compiler decides when to retain, borrow, transfer, and release values — none of this is left to codegen to infer (per §8, constraint 2: this is its own phase, producing an explicit IR that codegen reads mechanically).

### 9.1 Type categories

Stack types use copy semantics: `int32`, `int64`, `float64`, `bool`, `unit`.

Heap types use ownership semantics: `string`, `array`, `fn` closures, `Optional<T>` when `T` is heap-bearing, `Result<T, E>` when either payload is heap-bearing.

Custom structs are stack-only if all fields are stack types; heap-bearing if any field is heap-bearing.

### 9.2 Initialization

`let` and `var` declarations require an initializer. A variable is considered initialized only after its initializer has been semantically checked. Function parameters, loop variables, built-in namespaces, and function bindings are initialized when introduced. Reads from uninitialized variables are rejected by semantic analysis — the symbol table tracks initialization explicitly so that even future syntax (declaration without initializer) cannot silently create an uninitialized read path.

### 9.3 Mutability — separate from ownership

Mutability and ownership are independent axes. A binding or parameter can be borrowed-and-immutable, borrowed-and-mutable, owned-and-immutable, or owned-and-mutable — ownership answers "who releases this," mutability answers "can this be written through."

**One rule, two consequences.** `mut` grants write access through the binding. What that write is *observed by* depends entirely on whether the type is stack or heap — not on any special-cased syntax:

- **Stack types** (all of them are copy-semantics, §9.1, no exception): the write permission is local to the callee. The caller never observes it, because there was never anything shared to begin with — the parameter is a copy. `mut` here is purely a compile-time permission gate with **zero ABI difference** from an unannotated parameter; both are passed identically under the hood.
- **Heap types** (from v0.0.5 onward, once any exist): a write through a `mut` borrow reaches the caller's own storage, because the borrow is a reference to shared, owned data — there is no copy to isolate it behind.

```kai
type Point = { x: int32; y: int32; }

fn show(p: Point) { ... }                 // borrowed, immutable — cannot mutate p or its fields
fn touch(p: Point) { p.x = 5; }           // COMPILE ERROR — p is immutable
fn touch_mut(mut p: Point) { p.x = 5; }   // OK — permitted locally; caller's Point is unaffected (stack, copy semantics)
```

The realistic use case for field writes at v0.0.3 (before any heap type exists) is actually a `var` local, not a `mut` parameter — and that already works fully under copy semantics:

```kai
var p = Point { x: 0, y: 0 };
p.x = 5;   // fine — p is a local var, no parameter/borrow involved
```

Rules:
- `let` bindings cannot be reassigned and cannot have their fields mutated through them.
- `var` bindings may be reassigned; if heap-bearing, reassignment releases the old owned value after the replacement is prepared (§9.4).
- Function parameters are borrowed and **immutable by default**. A parameter must be declared `mut` to permit mutation of its contents (`p.x = 5` inside the function body) — without `mut`, this is a compile error, not a runtime borrow-check.
- `mut` on a parameter changes mutability only. It never changes ownership — a `mut` parameter is still borrowed; the callee never releases it at scope exit, and the caller remains the owner. For stack types this is entirely a static gate (§9.3 above); for heap types it additionally determines whether writes propagate to the caller's storage.

This is a hard requirement introduced at v0.0.2 scope (where `let`/`var` first exist) and extended to parameters at v0.0.3 (where function calls with params first exist) — mutability checking is not deferred to a later version, since retrofitting it has the same "touch everything at once" risk called out in §8.

**Generalization: `Place`, not per-construct special cases.** Whether a location can be written through is determined entirely by its *root* binding's mutability — field access and array indexing are both just **projections** of a `Place`, and every projection inherits the root's write permission uniformly. There is one rule, not "a struct-field rule" and "a separate array-index rule":

> A `Place` is writable iff its root binding is mutable. Writable roots: `var` locals, `mut` parameters. Non-writable roots: `let` locals, ordinary (non-`mut`) parameters. Field and index projections (`p.x`, `arr[i]`, `p.a.b`, `arr[i].x`, arbitrarily chained) preserve — never grant or revoke — the root's write permission. Assignment requires the destination `Place` to be writable.

| Expression | Root | Writable? |
|---|---|---|
| `p.x` | `var p` | yes |
| `arr[i]` | `var arr` | yes |
| `p.x` | `let p` | no |
| `arr[i]` | `let arr` | no |
| `p.x` | `mut p` (param) | yes |
| `arr[i]` | `mut arr` (param) | yes |

The root of a `Place` is found by stripping every projection down to the base identifier — this is a trivial resolver-phase helper (`root_of(place) -> Ident`), not a new concept.

**Arrays are unconditionally heap-bearing — including arrays of scalars — which makes `mut arr` behave differently from `mut` on an all-scalar struct.** §9.1 lists `array` as a heap type without qualification (unlike `Optional<T>`/`Result<T,E>`, which are heap-bearing only *when their payload is*) — an array's backing buffer needs heap allocation regardless of what it stores. This means a `mut` array parameter falls under this section's **heap** consequence even when the element type is a plain stack type:

```kai
fn touch_mut(mut p: Point) { p.x = 5; }         // caller's Point is UNCHANGED — Point is all-scalar (D1), copy semantics
fn set_first(mut arr: int32[]) { arr[0] = 42; } // caller's array IS CHANGED — array is heap-bearing regardless of element type
```

It's tempting to assume `int32[]`'s scalar element type means it should behave like `Point` (local-only mutation) — that assumption is wrong, and worth flagging explicitly because it's an easy mistake: **what determines the ownership regime is the container type, not the element type.**

**The formal invariant — two orthogonal axes, never conflated:**

> **Mutability is gated by the root `Place`; mutation *visibility* is determined by the root `Place`'s type category.**

```
is_writable(place) = is_writable(root(place))                    ← Axis 1: gates whether assignment is legal at all
visibility(place)   = mutation_regime(type_of(root(place)))       ← Axis 2: what that mutation actually does
                        stack-only  → D1 (local copy, invisible to caller)
                        heap-bearing → shared mutation (visible to caller)
```

`p.x` and `arr[0]` can both be writable `Place`s under the exact same Axis-1 rule (root is `var`/`mut`) while having completely different Axis-2 storage semantics (`Point` is stack-only → D1; `int32[]` is heap-bearing → visible). Neither axis needs to know about the other to do its job — writability never looks at the element type, and the storage-semantics decision never looks at whether the destination happens to be writable.

**Two wrong models to avoid, precisely because they look almost right:**
- ❌ `IndexPlace writable iff element type is mutable` — writability is never an element-type question; it's always `is_writable(root(place))`, full stop.
- ❌ `array<int32> behaves like Point because int32 is stack` — the element type is irrelevant to the container's own ownership category; `array` is unconditionally heap-bearing per §9.1 regardless of what it holds.

Element type only re-enters the picture for the *replacement operation itself* (retaining the new value if it's heap-bearing, per the ordering rule below) — never for deciding whether the assignment is legal in the first place. Keeping these two concerns separate is what lets `Place` stay one uniform rule instead of splintering into per-container special cases.

**Replacement into an already-owned `Place` — ordering matters, and this extends §9.4's existing rule to a new destination kind, not a parallel one.** §9.4 already specifies the safe order for `var` reassignment: prepare the replacement value, *then* release the old one. Array-element replacement is simply another kind of owning destination that rule already covers — `arr[i] = x` is a `Place` replacement exactly like `p = x`, just with an index projection instead of a bare identifier. No new ownership rule is introduced here; only the set of destination kinds §9.4 applies to grows. Here the ordering is safety-critical, not just tidy:

```kai
var arr = ["a", "b"];
arr[0] = arr[0];   // must be safe — the RHS can alias the very slot being replaced
```

The only correct order: **evaluate and prepare the RHS (retaining it if it's a borrowed reference) *before* releasing the old value at the destination slot.** Releasing first and evaluating the RHS second would read already-freed memory whenever the RHS aliases the destination — exactly the same class of bug as the `return s` retain-boundary bug in §9.5, just at a different transfer point. This applies uniformly to any `Place` assignment (struct field, array element, plain `var`), not just arrays — array indexing just makes the aliasing hazard concrete and easy to demonstrate.

### 9.4 Assignment and binding

```kai
let x = expr;
```
The expression is evaluated and ownership transfers into `x`.

```kai
let y = x;
var z = x;
```
If `x` is heap-bearing, the binding **retains** `x`. The new variable becomes a co-owner; the original remains valid.

Assigning into an existing heap-bearing `var` releases the old owned value after the replacement value has been prepared.

### 9.5 The ownership-transfer boundary (correctness rule)

This is the rule that v0.4.x got wrong and must be enforced from the version it first becomes reachable — **v0.0.5**, the new "Ownership runtime" slot, where `string` and arrays (the first heap-bearing types) actually exist. It does not apply at v0.0.3: struct/field mechanics land there, but with zero heap-bearing types active (all v0.0.3 structs are stack-only per §9.1), there is nothing for retain to ever insert — the rule is correct as written but has no exercisable case until v0.0.5.

> **Every time a *borrowed* reference — a parameter, a field access, an array element, or any binding still owned elsewhere — moves into a position that demands ownership (`return`, a struct-literal field, an array-literal element, assignment into an owning `var`/field), the compiler inserts an explicit retain at that point.** Only genuinely owned temporaries (the direct, unretained result of an expression with no other owner) may be moved without a retain.

Without this rule, the following double-frees silently:

```kai
fn id(s: string) -> string {
    return s;   // s is BORROWED, not a local owner — must retain before transferring out
}

let out = id(name);
// without the retain: name and out share one refcount: freed twice at scope exit
```

The same boundary applies anywhere a borrowed value is written into an owning slot, not just `return`:

```kai
fn wrap(s: string) -> User {
    return User { name: s };   // s borrowed; struct-literal field is an owning slot — retain required
}

fn pair(s: string) -> string[] {
    return [s, s];              // array-literal elements are owning slots — retain each
}
```

Compiler-facing summary table:

| Source expression | Destination | Behavior |
|---|---|---|
| Owned temporary | Any owning slot | Move, no retain |
| Borrowed binding (param, field access, array index) | Any owning slot (`return`, struct-literal field, array-literal element, `var`/field assignment) | **Retain**, then move |
| `let y = x` / `var z = x` where `x` heap-bearing | New binding | Retain, co-own |
| `foo(x)` | Function argument | Borrow, no retain |

This lives in its own IR-producing phase (§8's "ownership resolution," between typecheck and effect-check) — it is not something codegen infers. The output is an IR where every retain/release/move is already an explicit node; codegen never decides this itself, mirroring the same discipline as type information (§8, constraint 2).

**How this generalizes to structs with heap-bearing fields (per-field retain, not whole-struct refcounting).** Once `string` exists (v0.0.5), a struct with a `string` field becomes heap-bearing per §9.1 — but this does not turn the struct itself into a single refcounted heap block. This was already implicit in §9.8's wording ("releasing the struct releases those field**s**," and a retained field becomes "independent of `user`'s lifetime" — both phrasings only make sense under per-field ownership) and is now made explicit:

- **Representation is unchanged.** A heap-bearing struct is still a stack-resident (or embedded-in-parent) aggregate, laid out exactly like a stack-only struct — `%StructName = type { i32, ..., %KaiString* }`. A heap-bearing field is a pointer to a *separately* heap-allocated, refcounted buffer; the struct aggregate itself is never itself heap-allocated or refcounted as one unit.
- **Copy = memcpy the aggregate + retain each heap-bearing field individually.** Every transfer-boundary case in the table above (parameter passing of an owned temporary, `let`/`var` binding, `return`, struct-literal field-init) generalizes from "retain the one value" to "retain every heap-bearing field within the aggregate being moved," recursively for nested structs-of-structs.
- **Release = release each heap-bearing field individually**, not decrement one refcount for the whole struct — exactly as §9.8 already states, recursively for nested structs.
- **Why not whole-struct refcounting instead:** boxing the entire struct (one heap allocation, one refcount, for the aggregate as a whole) would give *every* field of that struct reference semantics the moment *any one* field is heap-bearing — including otherwise-plain `int32` fields, which would silently stop being copy-semantics (§9.1's blanket "stack types use copy semantics" would then have a hidden exception depending on a sibling field's type). That's a spooky-action-at-a-distance surprise this project has deliberately avoided everywhere else (D1's stack/heap split is exactly the opposite instinct — keep the axis local and type-determined, never structurally contagious). Per-field retention keeps every field's semantics locally determined by *that field's own type*, with no exception.
- **Interaction with D1 (§9.3):** unaffected. D1 only ever applied to structs where *every* field is a stack type (`Point`, `Segment` in the v0.0.4 fixtures). The moment a struct has any heap-bearing field (nested arbitrarily deep), it falls under this section's rules instead, and `mut` on such a parameter follows §9.3's heap-type consequence (writes are visible to the caller through the borrow) rather than D1's stack-type consequence (local-copy-only, zero ABI difference). This isn't a new special case — it's the same stack/heap axis §9.3 already established, just now reachable for the first time once `string` exists.

### 9.6 Function calls

Arguments are borrowed by default (see §9.3 for mutability of that borrow). The caller keeps ownership; the callee does not release the argument at scope exit as if it owned the caller's storage.

If a function returns a heap value, ownership transfers to the caller, subject to the retain rule in §9.5 — an owned local moves for free; a borrowed value being returned is retained first.

### 9.7 String literals

String literals are borrowed static storage at expression level. When stored into an owning location, the compiler creates an owned runtime string.

**v0.0.5 scope: plain string literals only — `"hello"`, `"hello world"`. No interpolation.** `${...}` expression embedding is deferred past v0.0.5 — it isn't just syntax sugar once you ask what it actually requires: evaluation order for embedded expressions, a defined conversion from arbitrary values to their string representation, and the ownership treatment of the resulting temporaries. None of that is specified yet, and v0.0.5 is already one cluster (string + array + the first exercise of retain/release, §7) — folding in a formatting/conversion feature on top would blur a version whose whole point is ownership, not string formatting. Interpolation returns once expression-to-string conversion semantics exist as their own decision (tracked in Appendix A) — not bundled into this version by default.

**String equality (`==`/`!=`) is content comparison, and is a borrow operation — never pointer identity, never an ownership event.**

```kai
let a = "hello";
let b = "hello";
a == b   // true — same content. Whether `a` and `b` happen to share one
         // allocation or point to two separate ones is never observable
         // through `==`; it must not be.
```

Two reasons this is a hard rule, not a style preference:

1. **Correctness.** A literal, a constructed string, a retained co-owner, and a value returned from a function can all represent the same content while having completely different allocation identities (§9.4–9.6). If `==` compared pointers, whether two equal-looking strings compare equal would depend on unrelated storage decisions the language has never promised to control — the same class of "leaking implementation detail into observable semantics" this project has refused everywhere else (retain-on-transfer, per-field struct retention, the `Place` writability/visibility split).
2. **Ownership discipline.** `==` only ever *borrows* both operands to compare bytes — it does not retain, release, or otherwise touch ownership. This matches every other read-only access already established: field access (§9.8) and array indexing (§9.9) are borrows, not ownership events, and equality comparison is no different — it reads, it doesn't own.

Interning or a pointer-equality fast path (`if lhs.ptr == rhs.ptr: true else: compare bytes`) is a legitimate implementation optimization *underneath* this rule, exactly because it's unobservable — the compiler is free to take that shortcut as long as the two paths always agree. What's fixed is the observable semantic: content equality, full stop.

### 9.8 Struct fields

The struct owns heap-bearing fields; releasing the struct releases those fields. Field access (`user.name`) is a borrow. Binding a heap-bearing field (`let name = user.name;`) retains it — `name` becomes a co-owner, independent of `user`'s lifetime.

### 9.9 Array elements

The array owns its elements. Indexing (`arr[0]`) borrows the element for immediate use; binding it retains it. Loop iteration (`for item in arr`) borrows each element per iteration — the array remains the owner of all elements after the loop.

### 9.9a Tagged unions — `Optional<T>`, `Result<T,E>` (v0.0.6)

`Optional<T>` and `Result<T,E>` are represented as tagged payloads — `{ tag, payload }` for `Optional`, `{ tag, payload: T | E }` for `Result` — not as two separately-implemented types. This is the same generalization already established for structs (§9.8, per-field) and arrays (§9.9, per-element), applied to a payload gated by a tag instead of a field name or index:

> For a tagged union, ownership operations (retain/release) apply only to the **active payload**, and only when that payload's **instantiated** type is heap-bearing. The tag itself carries no ownership semantics — it only determines which payload, if any, is live.

Concretely: `Optional<int32>`'s `Some`/`None` never retain or release anything (payload is a stack type). `Optional<string>`'s `Some` retains/releases its `string` payload exactly as any other heap-bearing binding would; `None` has no payload to touch. `Result<int32, Error>` only retains/releases on the `Err` branch (assuming `Error` is heap-bearing); `Result<string, int32>` only on the `Ok` branch. There is no separate implementation per instantiation or per payload category — one mechanism, gated by (a) which branch is active and (b) whether that branch's type happens to be heap-bearing, exactly mirroring §9.9a's structural sibling rules.

**`T?` is canonical source-level sugar for `Optional<T>`, not a second semantic form.** `string?`, `int32?`, `Foo?` desugar to `Optional<string>`, `Optional<int32>`, `Optional<Foo>` before typecheck ever sees them — the compiler never carries two distinct "nullable" concepts. `Result<T, E>` has no postfix sugar: a unary type parameter (`Optional`) justifies a unary sugar form; a binary one (`Result`) does not have a natural unary shorthand, so it stays fully explicit.

**Discarding a value of `Optional<T>` or `Result<T,E>` type is a diagnostic — for both, symmetrically — with one explicit escape hatch.** Once `Optional` carries real semantic information (`None` vs `Some`), silently ignoring it is exactly as dangerous as silently ignoring a `Result`'s error channel (§2.3): `lookup(key);` used as a bare statement, where `lookup` returns `Optional<T>`, is indistinguishable from a typo that meant to check the result. This applies to any statement-position expression whose static type is `Optional<T>` or `Result<T,E>`, not just direct calls — the `CallExprStmt`/bare-expression-statement production (grammar §6) is exactly where this check lives, at typecheck. The escape hatch is the discard statement (§9.9b) — never silent, always an explicit marker of intent.

### 9.9b Discard statement — `_ = expr;`

```kai
_ = maybe_value;
_ = result;
_ = some_side_effect();
```

`_ = expr;` is the one, canonical, explicit-discard form, introduced alongside the discard-diagnostic rule above (§9.9a) so that rule ships with its escape hatch in the same version, not a diagnostic with no idiomatic way to say "yes, I meant to ignore this." It applies uniformly to **any** expression type, not just `Optional`/`Result` — discarding is a general capability, the diagnostic requirement above is specifically what makes it *mandatory* for those two types.

Rules:
- `_` is reserved exclusively for this form. It is not a valid identifier — `let _ = expr;`, a `_` parameter name, or any other binding position using a bare `_` is rejected. Unlike Rust, there is no "discard pattern" usable anywhere a normal binding is; there is exactly one discard statement shape.
- The discarded expression is still evaluated normally, and its result still goes through ordinary lifetime/ownership rules (retain-if-needed, then immediate release) — `_ = expr;` performs no special ownership operation. It's an ordinary expression whose result simply isn't bound to anything.

### 9.10 Closure capture

**Every closure value is heap-bearing, unconditionally — regardless of what it captures, including a closure that captures nothing at all.** This mirrors array's already-established rule (§9.1, §9.3's Axis-2 discussion): the container's ownership category is a fixed property of the container type, never inferred from what it happens to hold. A closure that captures only a single `int32` is exactly as heap-bearing as one that captures a `string` — there is no "small capture, stays on the stack" special case. This is a semantic rule, not a missed optimization: a compiler is free to prove, as an *internal* optimization, that some particular closure never escapes its creating scope and skip the heap allocation — but that can never be *observable*, and the language-level ownership model always assumes heap allocation. Captured values themselves still follow their own ordinary ownership rules once inside the environment (a captured `string` is retained into the environment exactly as any other heap-bearing binding would be, §9.7).

Captured heap values are retained into the closure environment. Releasing the closure releases the environment; the environment destructor decrements its own reference count first, then releases captured heap fields only when the environment reaches zero.

**Closure reference cycles are rejected at compile time — a v0.0.6 invariant, not deferred.** Appendix A flagged self-referential closures as "known-unsound, must be flagged rather than silently accepted" while closures didn't exist yet; now that they do, that flag becomes an enforced rule, not a standing warning. The direct case (`let f = fn() { f(); };`) is already impossible under the existing initialization-order rule (§9.2: a variable cannot be read inside its own initializer) — but a cycle is still constructible indirectly, through a container the closure captures and is later stored back into:

```kai
type Node = { action: (unit) -> unit; }

var n = Node { action: someDefault };
n.action = fn() -> unit { n.action(); };   // captures `n`, then is stored into n's own field
```

Here `n` owns a field holding a closure whose environment captures `n` — a genuine cycle, and one the runtime's plain reference counting (§9, no cycle collector, §9.12) cannot reclaim.

**The rejection rule, made precise and decidable — not full alias analysis:**

> A closure literal may not capture a value whose type is, or transitively contains (as a heap-bearing member — a struct field, an array element type, an `Optional`/`Result` payload), a closure type.

This is checked purely structurally, reusing the `TypeDecl` dependency graph already built for cyclic-struct rejection (§3.3) — extended so that a closure-typed field or element **poisons** the containing type as "closure-bearing," transitively, exactly like the existing cycle-DFS already propagates through nested field types. A closure literal capturing anything of a closure-bearing type is rejected, regardless of whether that specific program actually writes the closure back into the poisoning field. This is deliberately over-conservative — it can reject some programs that would, in fact, never form a cycle — in exchange for being a decidable, purely type-level check with no new whole-program analysis machinery, consistent with §3.3's existing precedent: reject conservatively rather than accept a graph that might be cyclic.

This does **not** restrict closures from capturing heap-bearing values in general — that's the ordinary, expected case:

```kai
fn make_greeter(name: string) -> (unit) -> unit {
    return fn() -> unit { io.println(name); };   // captures `string` — fine, `string` is not closure-bearing
}
```

Only capturing a value whose type is itself closure-bearing is rejected — the rule targets the specific structural precondition for a cycle, not heap-bearing captures generally.

### 9.11 Scope exit

At scope exit, all owned heap-bearing locals introduced in that scope are released unless ownership was transferred out by `return` (per §9.5). Statement temporaries producing owned heap values are released at the end of the statement unless retained or transferred into an owning location.

### 9.12 Current non-goals

- No move-only user-visible syntax yet.
- No explicit reference syntax beyond borrowed parameters (§9.3).
- No user-defined destructors yet.
- No tracing garbage collector.
- No cycle collection for reference cycles (e.g. self-referential closures) — see Appendix A.

---

## 10. Runtime error model

Runtime traps are reserved for programmer errors that cannot be recovered from locally. Recoverable operations return `Optional` or `Result` instead (§3.4). This split is the runtime-level enforcement of §2.3 — a failure is either declared as recoverable in the type signature, or it is loud and terminal. There is no third, silent category.

### 10.1 Panic format

Runtime panics print to stderr and terminate the process with exit code `101` (matching Rust's panic convention, adopted deliberately for familiarity, not an arbitrary choice):

```
kai runtime panic: <message>
  at src/orders.kai:42:9
```

The source location is mandatory. Kai's diagnostics (`{ file, message, span, severity }`) are first-class from v0.0.1 (§8, constraint 6); a panic without a location would be the one error path in the language that doesn't benefit from that investment.

### 10.2 Current runtime panics

- Array index out of bounds.
- Structural array mutation while iterating.
- Integer division or modulo by zero.
- Signed `int32` overflow in `+`, `-`, or `*`.
- Signed `int32` division overflow, such as `INT32_MIN / -1`.
- Internal runtime allocation failure (§10.5).
- Negative `time.sleep_ms(...)` duration.
- `require` violation (§10.3).
- Contract drift detected at runtime (§10.6).

`observe` never panics — it is a Signal, not a Trust (§5.0, §5.2), and has no Violation by definition.

### 10.3 `require` violations panic — `observe` never does

A `require` violation traps, immediately, at the point of violation. This is unconditional — `require` is Correctness Trust (§5.0): it has exactly one Violation behavior, always fatal, with no statistical softening. That statistical framing now belongs entirely to `observe` (§5.2), which is not a Trust and never panics under any circumstance — the split exists precisely so that these two behaviors are never forced into one construct with ambiguous semantics again.

Sequencing on a `require` violation:
1. The violation is recorded to the debt ledger **before** the panic proceeds — concretely, a synchronous, flushed write to `.kai/debt.log` (JSONL, one record per line, mirroring `observe.log`'s sink design in §5.2.2 and the same location-without-project-root rule: documented no-op via `compile(&str)`, never CWD):
```
{"timestamp": "...", "location": "src/billing.kai:12:5", "kind": "correctness", "condition": "user.age > 0", "event": "violation"}
```
   `kind` matches §5.6's debt categories (`contract`/`temporal`/`correctness`/`reversibility`) so v0.0.12's `kai debt` aggregation can read this file directly with no format translation. This happens before the process exits, not asynchronously afterward, so a crash never loses the record.
2. The process panics per §10.1, with the required condition as the message:
```
kai runtime panic: requirement violated: user.age > 0
  at src/billing.kai:12:5
```

An `observe` recording a false condition does the opposite: it updates the Signal's history in `kai debt` (under Signals, not Debt — §5.6) and execution continues normally, with no trap of any kind.

### 10.4 Panics inside `reversible` functions — mandatory unwind, rollback vs. compensation

A panic occurring partway through a `reversible` function must not leave partially-applied effects in an undefined state. Before the process terminates, the runtime walks the accumulated effect history up to the point of the panic and unwinds it in reverse order — but *how* each step unwinds depends on its subtype (§5.3):

1. **Transactional mutations** are rolled back by restoring each mutated Place to the compiler-captured value it held immediately before the mutation. The snapshot is captured before the write and restored during reverse-order unwind. This is exact by construction: restoring the literal prior value has no case analysis per operator and no rounding risk.
2. **Compensating actions** (`compensate` blocks around external effects) are executed as declared — but this is a best-effort compensation, not a guaranteed undo. The runtime does not and cannot claim the external world has been restored to its prior state, only that the declared compensating action was attempted.
3. Only after all unwind steps have been attempted does the panic proceed to §10.1's terminal format and exit.

This makes the `reversible` guarantee hold on the error path for the transactional subtype, and makes the *attempt* (not a guarantee) explicit for the compensating subtype — the whitepaper no longer uses "rollback" to describe both, since conflating them overstates what actually happens to an already-sent email or an already-fired webhook (§5.3). If a transactional restore or a compensating action itself fails (e.g. the snapshot restore write fails, or `refundCard` in a `compensate` block throws), that is treated as a distinct, more severe trap — `kai runtime panic: rollback failed after partial transfer` for the transactional case, `kai runtime panic: compensation failed after partial transfer` for the compensating case — rather than silently giving up. An unrecoverable unwind must still be loud, and the message must not falsely imply a guarantee that didn't hold.

### 10.5 Numeric policy

- `int32` arithmetic is checked for `+`, `-`, `*`, `/`, and `%`. Overflow traps instead of wrapping silently.
- `float64` follows LLVM/libc floating-point behavior and does not trap on overflow or division by zero (produces `inf`/`NaN` per IEEE 754, as usual for this type).

### 10.6 Contract drift detected at runtime

`dsl sql` / `dsl api` blocks are validated against a build-time **snapshot** (§5.4), not a live connection. If the live schema or API has drifted further since the last `kai sync` — and a query's actual result no longer matches the shape the compiler verified — this is neither a classic programmer error (the code was correct against the snapshot it had) nor an ordinary recoverable failure (it isn't "file not found"; it's a contract silently broken). It is a distinct, terminal category:

```
kai runtime panic: contract drift detected at runtime
  snapshot sql(v12) no longer matches live schema for column 'orders.total'
  at src/orders.kai:9:5
```

This trap is automatically recorded as a HIGH-severity entry in `kai debt` — this is exactly the situation the debt ledger (§5.6) exists to surface, so it is captured at the moment it's proven true (runtime), not just estimated at build time from snapshot age.

### 10.7 Recoverable errors

Stdlib operations that can reasonably fail at runtime return `Result` or `Optional`:

- `fs.read_to_string(path) -> Result<string, string>`
- `fs.write(path, content) -> Result<unit, string>`
- `env.get(name) -> string?`
- `str.parse_int(value) -> Result<int32, string>`

### 10.8 Allocation policy

Runtime-managed heap allocations are treated as internal infrastructure. If `malloc`/`realloc` returns null for strings, arrays, closure environments, or runtime-owned buffers, Kai traps:

```
kai runtime panic: out of memory
```

Stdlib operations still return `Result` for normal recoverable failures — missing files, failed opens, partial reads/writes. Allocation failure itself is not currently modeled as a recoverable `Result` payload; it is treated as unrecoverable infrastructure failure, consistent with §2.3's split (this is not a case the caller can meaningfully handle locally).

---

## Appendix A — Open questions (deliberately unresolved)

These are known unresolved design questions. They are *not* to be decided ad-hoc mid-implementation — they get resolved here, in this document, first.

- ~~`T` vs. `T @local(d)` — exact-type mismatch, no strip operation exists.~~ **Resolved (§5.1.3a), verified by 13 `v007_*` tests.** `T @local(d)` may be read as plain `T` at an argument position (marker-drop on read, not a representation conversion — the modifier stays in the TAST node). Propagation safety is decided by the effect checker, not typecheck: a non-escaping callee accepts it silently, an escaping callee still rejects it via the proper boundary diagnostic. Confirms `@local` is not contagious through non-escaping code, matching its zero-runtime-footprint design intent.

- **`${...}` string interpolation — deferred past v0.0.5 (§9.7).** Needs its own decisions before it can land: evaluation order for embedded expressions, a defined conversion from arbitrary values to their string representation (does every type get one? via what mechanism — a trait-like interface, or a closed set of built-ins?), and the ownership/temporary-lifetime treatment of the resulting formatted pieces. None of this is a small addition to `StringLit`'s lexical grammar (which already has the `${...}` shape defined, unused) — it's a small conversion sub-language of its own. No target version yet.

- Are `require`, `reversible`, `@local`/`@wallclock` part of the type system (checked, can reject a program) or purely annotations read by tooling? This determines whether an effect-tracking layer is required in the type checker. (Note: regardless of the answer, their runtime failure behavior is now fixed — §10.3, §10.4.)
- Conflict resolution when two overrides on the same field disagree over time — last-write-wins with a flagged history, or hard block until reconciled?
- ~~Where does `observe`'s history report to~~ **Resolved (§5.2.2).** Pluggable sink interface, one v0.0.8 implementation: local append-only JSONL file (`.kai/observe.log`), no telemetry/network sink. Full `kai debt` dashboard aggregation remains v0.0.12 scope (§7), unaffected by this — v0.0.8 only needed the raw recording mechanism decided.
- Severity heuristics for `kai debt` — fully compiler-inferred, fully config-driven, or hybrid (default + override)? Leaning hybrid; needs a concrete default rule set written down before v0.0.12.
- ~~Precise definition of "compiler-untraceable boundary"~~ **Resolved (§5.1.1–§5.1.3).** Defined mechanically as the `escapes-local-context` effect — loss of statically provable execution-context continuity — not a hardcoded construct list; inferred transitively over the call/closure graph (least fixed point over SCCs), verified against optional `effects { ... }` contract annotations.
- ~~`@wallclock` serialization format.~~ **Resolved (§5.1.5).** Canonical RFC 3339, UTC only, fixed microsecond precision, mandatory `Z` suffix — no local offsets. `deserialize(serialize(t)) == t` invariant.
- **Boxing/indirection mechanism — undesigned.** §3.3 rejects cyclic struct definitions outright (compile error, no exception), but Kai has no way to legitimately express a self-referential type (linked list, tree, recursive enum) once one is actually wanted. Needs a design before any real program that needs such a structure can be written — likely a `Box<T>`-equivalent introducing one level of heap indirection, but the mechanism, its interaction with the ownership model (§9), and which version it belongs to are all open.
- ~~Discarding an `Optional`~~ **Resolved at v0.13, implemented v0.0.6.2.** Symmetric with `Result` — both require a diagnostic when discarded silently (§9.9a); `_ = expr;` is the escape hatch. `Optional` carries real semantic information (`None` vs `Some`) once it exists, making silent discard exactly as dangerous as swallowing a `Result`'s error channel — not lower-risk after all. Verified: 295 passing tests, `crates/kai-typecheck/src/expr/tagged.rs`.
- **Reference cycles — general case, still open.** §9.10's closure-cycle rule handles the closure-specific case conservatively via type-level poisoning, but the general cycle-collection question (§9.12) remains open: pure RC (§9) still leaks on any cycle the poisoning rule doesn't happen to reject (e.g. two plain structs holding `Optional<Box<...>>`-style references to each other, once such indirection exists — see the boxing/indirection item above, still undesigned). Candidate directions unchanged from before: a `weak` reference kind, or a narrow opt-in cycle collector. Needs a decision before general self-referential data structures are considered supported.
- **Decay taxonomy — proposed, not yet adopted.** §5.0 defines Decay as a required field of `Trust⟨C⟩` but does not currently classify *kinds* of Decay; each Trust instance just names its own mechanism in prose. A candidate taxonomy worth evaluating: `temporal` (time passes — Temporal Trust), `external` (an outside authority changes — Contract Trust), `stateful` (in-process world state changes — Correctness Trust), `invalidation` (a specific event revokes the claim — candidate fit for Reversibility, though its Decay is currently left as "—" in §5.0's table and may not need one at all). If this holds up under scrutiny, it sharpens Trust's definition from "a claim with a confidence level" to "a claim with a defined expiration mechanism," which is a stronger and more specific claim than the current draft makes. This is deliberately not promoted into §5.0 yet — it needs to be checked against each of the four instances (including the Transactional/Compensatable split) before it's treated as load-bearing, not just descriptive.

- **Memory hardening framework — proposed, not yet adopted.** Two tiers, not one:

  **Tier 1 — Immediate (v0.0.8.6, no amandemen needed).** Move existing leak isolation artifacts from `/tmp/opencode/leak/` to permanent `tests/fixtures/` fixtures. These are proven regression tests, not design proposals — they belong in CI now, not in a discussion document. Concretely:
  - `stress_heap.kai` (40 heap ops/iter × 10 iters, ASan-verified 0 leaks) → `tests/fixtures/leak/stress_heap.kai`
  - `minimal.kai` (standalone `&&` rhs leak) → `tests/fixtures/leak/minimal.kai`
  - `minimal2.kai` (`&&` inside `for` loop) → `tests/fixtures/leak/minimal2.kai`
  - `test3.kai`–`test5.kai` (progressive isolation: loop → loop+Result → loop+catch+closure) → `tests/fixtures/leak/test3.kai`–`test5.kai`
  - Pattern: each file JIT-compiles and asserts an exit code (existing golden-IR convention). ASan build (`CARGO_TARGET_DIR=target-asan`) runs the full corpus including these fixtures on every PR. No new tooling, no whitepaper change — pure test promotion.
  - ASan CI gate: `ASAN_OPTIONS=detect_leaks=1` on `kai-ownership`, `kai-codegen`, `kai-driver` test suites, every PR. Stale-binary guard: always rebuild with `CARGO_TARGET_DIR=target-asan`, never reuse a cached binary.

  **Tier 2 — Deferred (v0.0.12, needs amandemen before implementation).** The "proper" production standard — heap profiling with trend analysis, static lifetime/ownership checking in the compiler, input fuzzing, concurrency race detection, diagnostic-quality leak reports (WHAT/WHERE/WHO/WHEN/WHY/PATH). This is the framework that maps onto `kai debt`'s Trust⟨C⟩ projection model and belongs alongside it. **Not adopted yet** — needs discussion on: (a) whether ASan-on-every-PR is CI-budget-acceptable or should be main/nightly-only; (b) whether P2 diagnostic standard requires new `kai` CLI subcommands; (c) whether panic-path leaks (§10.1 `process::exit(101)` without unwind) are intentional limitations or must be fixed by v0.0.9's `reversible` unwind mechanism.

  **Production standard (for Tier 2 adoption):**
  - **P0 — Hard failure (must be 0):** use-after-free, double-free, invalid memory access, ownership violation, unhandled leak on ephemeral objects, crash from lifecycle violation.
  - **P1 — Memory stability:** on representative workload, heap reaches steady state; no sustained growth; retained objects have valid owners; caches/pools have stated bounds.
  - **P2 — Diagnostic quality:** failing test answers WHAT/WHERE/WHO/WHEN/WHY/PATH. "Memory increased by 47 MB" is not a diagnostic.
  - **P3 — Regression resistance:** memory tests are CI gates, not manual pre-release checks.