# Kai
### A trust-aware programming language

**Status:** Draft v0.13 — pre-implementation specification
**Purpose:** Freeze scope before writing any compiler code. Nothing described here is authoritative until it appears in this document. Feature ideas that arise during implementation go into an `IDEAS.md` backlog, not into the compiler.

**Amendment process:** Small additions (new syntax sugar, clarifying rationale) may be edited directly. Anything touching §2 (principles), §4 (non-goals), or introducing a new Trust kind beyond §5.0's taxonomy must first exist as an entry in Appendix A, be discussed explicitly, and only then be promoted into the main body — never patched in ad hoc during implementation.

**Changelog**
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
let absent: string? = None;
let fallback: string = maybe_name ?? "unknown";

let parsed: Result<int32, string> = str.parse_int("42");
```

**Change from v0.4.5:** `??` is reserved for `Optional` only. `Result` requires an explicit, non-silent unwrap:

```kai
let value: int32 = parsed.unwrap_or(0);
let value: int32 = parsed catch |err| { io.eprintln(err); 0 };
```

**Construction (v0.0.6): `Some(expr)` and bare `None` are the only Optional constructors.** `None` carries no payload, so it has nothing to infer from — like the empty array literal, it is legal only where a context type fixes `T` (`let x: string? = None;` is valid; a bare `let x = None;` is a typecheck error). Same mechanism as arrays — annotation, parameter/return position, or an outer typed literal — no new inference machinery.

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
- **v0.0.4's own tests don't need the stdlib.** Module resolution, qualified calls, `public` visibility, and circular-import detection are all fully exercisable with user-defined modules alone (e.g. a local `support/math.kai` with `public fn add(a: int32, b: int32) -> int32`). The stdlib itself is deferred to v0.0.5 (§3.7) — implementing any of it now against types that don't exist yet would just be thrown-away work.

### 3.7 Standard library (built-in, no disk resolution)

**Version note:** deferred to v0.0.5. Every stdlib signature here depends on `string` and/or arrays, neither of which exist before v0.0.5 (Ownership runtime, §7) — implementing this earlier would be discarded once those types land.

| Import | Surface |
|---|---|
| `std.io` | `println`, `print`, `eprintln`, `readln` |
| `std.fs` | `exists`, `read_to_string`, `write`, `append`, `remove`, `rename` |
| `std.env` | `get`, `cwd` |
| `std.str` | `parse_int`, `parse_float`, `join` |
| `std.math` | `sqrt`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `pow`, `abs`, `min`, `max` |
| `std.time` | `now`, `millis`, `sleep_ms` |

Everything above this line must be fully specified, implemented, and tested before Section 5 begins. This is the scope boundary for v0.0.1–v0.0.6 (the new v0.0.5 ownership-runtime slot plus what was v0.0.5 and is now v0.0.6).

---

## 4. Non-goals (explicit, to prevent scope creep)

- Kai is not a general distributed-systems protocol. (See prior exploration of NEBULA/pub-sub — out of scope for the language itself; could be a future std module at most.)
- Kai is not trying to replace Rust/Go for systems programming, embedded, or performance-critical compute. Its target is backend services with heavy external integration surface.
- The effect system (§5) is not a general-purpose algebraic effects system à la Koka. It covers exactly the instances of the Trust taxonomy defined in §5.0 (Contract, Temporal, Correctness, Reversibility) plus Signal (§5.2, non-Trust telemetry). New effect or Trust kinds require a written amendment to this document, not an ad-hoc addition during implementation.
- `dsl sql` / `dsl api` are not a full ORM or query builder. They validate hand-written queries against a schema/spec snapshot; they do not generate queries.

---

## 5. Trust-aware layer (v0.0.7+ scope — built only after Section 3 is complete and tested)

This is opt-in. None of it changes how a function looks unless the function actually depends on external trust.

### 5.0 What "Trust" formally means

Kai's position is not "a language with several safety features." It is: **external trust is a thing to be modeled, and every trust-aware feature is an instance of one abstract structure, not a coincidence of four features living in the same compiler.**

```
Trust⟨C⟩ = (Claim, Origin, Verification, Decay, Violation)
```

- **Claim** — a proposition about `C` that the code depends on, which the compiler cannot guarantee from the code alone.
- **Origin** — the authority the claim rests on: a schema snapshot, the wall clock, an accumulated history of observation, a declared inverse operation.
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
| Reversibility — Transactional (§5.3) | mutation has a true inverse (`state_after + inverse = state_before`) | declared or automatically derived inverse operation | — | rollback failure is a distinct trap |
| Reversibility — Compensatable (§5.3) | mutation has an explicit compensating action (`state_after + compensator ≈ acceptable_new_state`, not a true inverse) | declared `compensate` block | — | compensation failure is a distinct trap |
| *(Signal, not Trust)* `observe` | none — pure observation | history of observed values | — | none; has no Violation, so it does not qualify as Trust at all |

`reversible` remains the single user-facing keyword — a function marked `reversible` may contain both Transactional and Compensatable effects — but the compiler internally treats them as two distinct Trust kinds with two distinct guarantees, precisely so the keyword never promises a stronger guarantee (true inverse) than what a given effect can actually provide (best-effort compensation). This is why "rollback" and "compensation failed" are reported as different trap messages in §10.4, not a single generic "reversal failed."

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

**Rule, enforced statically:** the moment a value typed `@local` would cross a boundary the compiler cannot trace through control flow — being sent to a queue, serialized, passed across a thread/task boundary, or stored beyond the current call graph — using it as `@local` is a compile error. It must be `@wallclock` at that point instead, which carries its own embedded timestamp and is checked against the real clock at the point of use, not against compiler-traced position. This turns the false-safety risk into a build-time rule rather than a silent gap: you cannot accidentally treat a value that left the process's traceable flow as if the compiler were still watching it.

### 5.2 Correctness and observation — `require` and `observe`

The original single `assume` construct tried to be two things that turned out to have incompatible semantics: a hard invariant (violating it is always fatal, so a confidence score attached to it changes nothing about program behavior) and a statistical observation (which, if it's ever allowed to be fatal, isn't really "statistical" at all). Kai splits these into two constructs — only one of which is a Trust.

**`require` — Correctness Trust.** Declares an invariant the function's correctness genuinely depends on. Violation always panics, unconditionally — there is no confidence score, because a confidence score would not change what happens when it's false.

```kai
fn getDiscount(user: User) -> Percent {
    require user.age > 0;
    ...
}
```

**`observe` — Signal, not Trust.** Pure telemetry: records how often a condition holds, purely for visibility, and never affects control flow or panics. Because it has no Violation, it does not fit the Trust structure (§5.0) at all — it is not tracked in `kai debt` as debt (nothing is owed), only surfaced as informational history.

```kai
fn getDiscount(user: User) -> Percent {
    observe user.age > 0;   // logged, never fatal, purely observational
    ...
}
```

If a `require` invariant should instead be soft-tracked for now while a fix is planned, that is what `@override` (§5.5) with `kind: "suppress"` and a mandatory expiry is for — not a weaker version of `require` itself.

### 5.3 Reversibility — `reversible`, and why "rollback" isn't always the right word

A function marked `reversible` must have every mutation inside it be either automatically invertible or explicitly paired with a manual reversal — but these are not the same guarantee, and conflating them was a real gap in the original draft. Database-style rollback is atomic and guaranteed once it runs: `state_after + inverse = state_before`, exactly. A compensating action against an external side effect (an email already sent, a webhook already fired) is not — it is a best-effort action that can itself fail, and it only reaches `state_after + compensator ≈ acceptable_new_state`, an approximation, not a true inverse. Kai keeps these as two distinct Reversibility subtypes at the semantic level (§5.0's table: Transactional vs. Compensatable) and reserves "rollback" for the first one only.

```kai
fn transferMoney(from: Account, to: Account, amt: Money) reversible {
    from.balance -= amt;   // transactional — automatic inverse, "rollback" is accurate here
    to.balance += amt;
}

transferMoney(a, b, 100).rollback();
```

```kai
fn onboardUser(user: User, fee: Money) reversible {
    db.insert(user);                              // transactional — automatic inverse

    sendEmail(user.email) compensate {              // compensating — never automatic, must be declared
        sendEmail(user.email, "disregard previous email")
    }

    chargeCard(user, fee) compensate {
        refundCard(user, fee)                       // still just a best-effort action, not a guarantee
    }
}
```

The compiler never generates a compensating action automatically — unlike arithmetic inverses, there is no general rule for "the opposite of sending an email," so it must always be authored by hand. Anything with neither an automatic inverse nor a declared `compensate` block is a compile error, not a silent gap (unchanged from the original rule — only the vocabulary around external effects has changed).

If a `reversible` function panics partway through, the mutations already applied are not left dangling — see §10.4 for the mandatory unwind behavior, which now distinguishes rollback (transactional) from compensation (external, best-effort, and itself fallible).

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
| v0.0.6 | `Optional`, `Result`, closures, `_ = expr;` discard statement | Tagged-union ownership (§9.9a) — active-payload-only, heap-bearing-only retain/release, one mechanism for both types; `T?` desugars to `Optional<T>` with no second semantic form; `.unwrap_or()` works on both `Optional`/`Result`, `catch` stays `Result`-only; discarding `Optional`/`Result` as a bare statement is a diagnostic, `_ = expr;` is the sole escape hatch (§9.9b); closures unconditionally heap-bearing regardless of capture (§9.10); closure-cycle rejection enforced via closure-bearing-type poisoning over the existing `TypeDecl` DFS graph (§9.10, extends §3.3) |
| v0.0.7 | `@local`, `@wallclock`, temporal flow analysis, cross-boundary rule | Compile error enforced when `@local` crosses a compiler-untraceable boundary (§5.1) |
| v0.0.8 | `require`, `observe` | `require` violation always panics per §10.3, recorded to debt ledger before exit; `observe` never panics, tracked as Signal only, not debt |
| v0.0.9 | `reversible` (transactional + `compensate`) | Automatic invertibility for arithmetic mutations; mandatory unwind on panic per §10.4, distinguishing rollback from compensation |
| v0.0.10 | `dsl sql` + snapshot mechanism | `kai sync` for at least one DB (e.g. Postgres) |
| v0.0.11 | `dsl api` + OpenAPI sync | |
| v0.0.12 | `@override` + `kai debt` unified ledger | |

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
1. The violation is recorded to the debt ledger (§5.6, `[correctness]`) — this happens before the process exits, not asynchronously afterward, so a crash never loses the record.
2. The process panics per §10.1, with the required condition as the message:
```
kai runtime panic: requirement violated: user.age > 0
  at src/billing.kai:12:5
```

An `observe` recording a false condition does the opposite: it updates the Signal's history in `kai debt` (under Signals, not Debt — §5.6) and execution continues normally, with no trap of any kind.

### 10.4 Panics inside `reversible` functions — mandatory unwind, rollback vs. compensation

A panic occurring partway through a `reversible` function must not leave partially-applied effects in an undefined state. Before the process terminates, the runtime walks the accumulated effect history up to the point of the panic and unwinds it in reverse order — but *how* each step unwinds depends on its subtype (§5.3):

1. **Transactional mutations** (automatic inverses, e.g. `+=` → `-=`) are rolled back — this is a real, guaranteed rollback, because the inverse is mathematically exact.
2. **Compensating actions** (`compensate` blocks around external effects) are executed as declared — but this is a best-effort compensation, not a guaranteed undo. The runtime does not and cannot claim the external world has been restored to its prior state, only that the declared compensating action was attempted.
3. Only after all unwind steps have been attempted does the panic proceed to §10.1's terminal format and exit.

This makes the `reversible` guarantee hold on the error path for the transactional subtype, and makes the *attempt* (not a guarantee) explicit for the compensating subtype — the whitepaper no longer uses "rollback" to describe both, since conflating them overstates what actually happens to an already-sent email or an already-fired webhook (§5.3). If a transactional rollback or a compensating action itself fails (e.g. the inverse operation also panics, or `refundCard` in a `compensate` block throws), that is treated as a distinct, more severe trap — `kai runtime panic: rollback failed after partial transfer` for the transactional case, `kai runtime panic: compensation failed after partial transfer` for the compensating case — rather than silently giving up. An unrecoverable unwind must still be loud, and the message must not falsely imply a guarantee that didn't hold.

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

- **`${...}` string interpolation — deferred past v0.0.5 (§9.7).** Needs its own decisions before it can land: evaluation order for embedded expressions, a defined conversion from arbitrary values to their string representation (does every type get one? via what mechanism — a trait-like interface, or a closed set of built-ins?), and the ownership/temporary-lifetime treatment of the resulting formatted pieces. None of this is a small addition to `StringLit`'s lexical grammar (which already has the `${...}` shape defined, unused) — it's a small conversion sub-language of its own. No target version yet.

- Are `require`, `reversible`, `@local`/`@wallclock` part of the type system (checked, can reject a program) or purely annotations read by tooling? This determines whether an effect-tracking layer is required in the type checker. (Note: regardless of the answer, their runtime failure behavior is now fixed — §10.3, §10.4.)
- Conflict resolution when two overrides on the same field disagree over time — last-write-wins with a flagged history, or hard block until reconciled?
- Where does `observe`'s history report to — local file, opt-in telemetry, or pluggable sink? Needs a decision before v0.0.8.
- Severity heuristics for `kai debt` — fully compiler-inferred, fully config-driven, or hybrid (default + override)? Leaning hybrid; needs a concrete default rule set written down before v0.0.12.
- **Precise definition of "compiler-untraceable boundary"** for §5.1's `@local`→`@wallclock` rule. Queue sends and explicit serialization are clear cases; less clear: does spawning an async task count, does an in-process channel count, does a thread pool hand-off count? Needs an exact, exhaustive list (or a structural rule the compiler can apply generally) before v0.0.7.
- **`@wallclock` serialization format.** The embedded timestamp needs a concrete wire representation once a `Token @wallclock(...)` is serialized (for the queue-send case in §5.1) — needs a decision before v0.0.7, likely tied to whatever `dsl api`/`dsl sql` end up using for payload encoding.
- **Boxing/indirection mechanism — undesigned.** §3.3 rejects cyclic struct definitions outright (compile error, no exception), but Kai has no way to legitimately express a self-referential type (linked list, tree, recursive enum) once one is actually wanted. Needs a design before any real program that needs such a structure can be written — likely a `Box<T>`-equivalent introducing one level of heap indirection, but the mechanism, its interaction with the ownership model (§9), and which version it belongs to are all open.
- ~~**Discarding an `Optional`**~~ **Resolved at v0.13 (§9.9a): diagnostic, symmetrically with `Result`.** The "lower-risk in principle" reasoning did not survive contact with the concrete case — once `Optional` carries real semantic information (`None` vs `Some`), silently ignoring it is exactly as dangerous as swallowing a `Result`'s error channel (`lookup(key);` is indistinguishable from a typo either way). The escape hatch ships in the same version: `_ = expr;` (§9.9b). This entry is retained as resolution history per the amendment process; it is no longer an open question.
- **Reference cycles — general case, still open.** §9.10's closure-cycle rule handles the closure-specific case conservatively via type-level poisoning, but the general cycle-collection question (§9.12) remains open: pure RC (§9) still leaks on any cycle the poisoning rule doesn't happen to reject (e.g. two plain structs holding `Optional<Box<...>>`-style references to each other, once such indirection exists — see the boxing/indirection item above, still undesigned). Candidate directions unchanged from before: a `weak` reference kind, or a narrow opt-in cycle collector. Needs a decision before general self-referential data structures are considered supported.
- **Decay taxonomy — proposed, not yet adopted.** §5.0 defines Decay as a required field of `Trust⟨C⟩` but does not currently classify *kinds* of Decay; each Trust instance just names its own mechanism in prose. A candidate taxonomy worth evaluating: `temporal` (time passes — Temporal Trust), `external` (an outside authority changes — Contract Trust), `stateful` (in-process world state changes — Correctness Trust), `invalidation` (a specific event revokes the claim — candidate fit for Reversibility, though its Decay is currently left as "—" in §5.0's table and may not need one at all). If this holds up under scrutiny, it sharpens Trust's definition from "a claim with a confidence level" to "a claim with a defined expiration mechanism," which is a stronger and more specific claim than the current draft makes. This is deliberately not promoted into §5.0 yet — it needs to be checked against each of the four instances (including the Transactional/Compensatable split) before it's treated as load-bearing, not just descriptive.