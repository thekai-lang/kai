# Kai
### A trust-aware programming language

**Status:** Draft v0.3 — pre-implementation specification
**Purpose:** Freeze scope before writing any compiler code. Nothing described here is authoritative until it appears in this document. Feature ideas that arise during implementation go into an `IDEAS.md` backlog, not into the compiler.

**Amendment process:** Small additions (new syntax sugar, clarifying rationale) may be edited directly. Anything touching §2 (principles), §4 (non-goals), or introducing a new Trust kind beyond §5.0's taxonomy must first exist as an entry in Appendix A, be discussed explicitly, and only then be promoted into the main body — never patched in ad hoc during implementation.

**Changelog**
- **Implementation note (unversioned)** — Compiler releases (`kai build --version`) carry their own `v0.X.Y` line, independent of this document's spec versions. Status of the `v0.0.2` implementation against this spec: bindings, stack primitives, arithmetic, `if`/`else`, boolean logic, and assignment statements compile end-to-end. One known divergence from §10.2: the arithmetic panic checks (integer division/modulo by zero, signed `int32` overflow including `INT32_MIN / -1`) are **specified but not yet emitted** — codegen runs plain unchecked LLVM ops until the runtime-trap slot lands; `float64` already follows the IEEE `inf`/`NaN` clause. Hardening on this line so far: parser recursion budget with poisoned recovery nodes, entry-block alloca placement, precise malformed-literal diagnostics, and stable duplicate-binding ids.
- **v0.3.1** — Grammar decisions locked during v0.0.2 implementation: boolean logic operators (`&&`, `||`, `!`) confirmed in the core language, with `&&` binding tighter than `||` and both short-circuiting; unary minus/NOT generalized (`UnaryExpr ::= ('-' | '!') UnaryExpr`); assignment made statement-only with an explicit assignable-place rule (identifier in v0.0.2); `string` deferred until the §9 ownership runtime exists; variable shadowing allowed across nested blocks, rejected within one scope. See `kai-ebnf.md` §1/§7 for the locked precedence chain.
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

## 3. Core language (v0.0.1 – v0.0.5 scope)

This section is intentionally boring. It is Rust/Go-adjacent on purpose — the interesting ideas live in Section 5, not here.

### 3.1 Hello world

```kai
use std.io;

fn main() -> int32 {
    io.println("Hello, Kai!");
    return 0;
}
```

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

### 3.3 Structs

```kai
type User = {
    id: int32;
    name: string;
}

let user = User { id: 1, name: "Kai" };
```

### 3.4 Arrays, Optionals, Results

```kai
let values: int32[] = [1, 2, 3];

let maybe_name: string? = Some("Kai");
let fallback: string = maybe_name ?? "unknown";

let parsed: Result<int32, string> = str.parse_int("42");
```

**Change from v0.4.5:** `??` is reserved for `Optional` only. `Result` requires an explicit, non-silent unwrap:

```kai
let value: int32 = parsed.unwrap_or(0);
let value: int32 = parsed catch |err| { io.eprintln(err); 0 };
```

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

- `use a.b;` resolves to `a/b.kai` from project root.
- Path segments `.`, `..`, `/`, `\` are rejected.
- Circular imports are a diagnostic, not a silent stack overflow.
- `public fn` is visible through the module alias; plain `fn` is module-private.
- Imports never inject into global scope — always namespace-qualified.

### 3.7 Standard library (built-in, no disk resolution)

| Import | Surface |
|---|---|
| `std.io` | `println`, `print`, `eprintln`, `readln` |
| `std.fs` | `exists`, `read_to_string`, `write`, `append`, `remove`, `rename` |
| `std.env` | `get`, `cwd` |
| `std.str` | `parse_int`, `parse_float`, `join` |
| `std.math` | `sqrt`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `pow`, `abs`, `min`, `max` |
| `std.time` | `now`, `millis`, `sleep_ms` |

Everything above this line must be fully specified, implemented, and tested before Section 5 begins. This is the scope boundary for v0.0.1–v0.0.5.

---

## 4. Non-goals (explicit, to prevent scope creep)

- Kai is not a general distributed-systems protocol. (See prior exploration of NEBULA/pub-sub — out of scope for the language itself; could be a future std module at most.)
- Kai is not trying to replace Rust/Go for systems programming, embedded, or performance-critical compute. Its target is backend services with heavy external integration surface.
- The effect system (§5) is not a general-purpose algebraic effects system à la Koka. It covers exactly the instances of the Trust taxonomy defined in §5.0 (Contract, Temporal, Correctness, Reversibility) plus Signal (§5.2, non-Trust telemetry). New effect or Trust kinds require a written amendment to this document, not an ad-hoc addition during implementation.
- `dsl sql` / `dsl api` are not a full ORM or query builder. They validate hand-written queries against a schema/spec snapshot; they do not generate queries.

---

## 5. Trust-aware layer (v0.0.6+ scope — built only after Section 3 is complete and tested)

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
| v0.0.3 | `type` structs, struct literals, function calls with params, `mut` parameters | Type checker does real signature/field matching; ownership-transfer retain rule (§9.5) enforced for `return`/struct-literal/array-literal; parameter mutability (§9.3) rejected without `mut` |
| v0.0.4 | `use` / module system | Circular import detection tested |
| v0.0.5 | `Optional`, `Result`, closures | Generic/parametric typing works |
| v0.0.6 | `@local`, `@wallclock`, temporal flow analysis, cross-boundary rule | Compile error enforced when `@local` crosses a compiler-untraceable boundary (§5.1) |
| v0.0.7 | `require`, `observe` | `require` violation always panics per §10.3, recorded to debt ledger before exit; `observe` never panics, tracked as Signal only, not debt |
| v0.0.8 | `reversible` (transactional + `compensate`) | Automatic invertibility for arithmetic mutations; mandatory unwind on panic per §10.4, distinguishing rollback from compensation |
| v0.0.9 | `dsl sql` + snapshot mechanism | `kai sync` for at least one DB (e.g. Postgres) |
| v0.0.10 | `dsl api` + OpenAPI sync | |
| v0.0.11 | `@override` + `kai debt` unified ledger | |

Anything not on this table is out of scope until this document is amended.

---

## 8. Compiler implementation constraints

(Non-negotiable, independent of language design — this is what killed v0.4.x.)

Toolchain carried over from v0.4.5, kept deliberately: hand-written recursive-descent parser, inkwell/LLVM for codegen. These were never the problem. The problem was a missing boundary between "untyped AST" and "codegen input" — raw AST reached codegen directly, so when a real type system was retrofitted later, every codegen call site had to be revisited at once, mid-flight, with no way to land it incrementally. That failure mode is the single most important thing this section exists to prevent.

1. **Typed AST (TAST) is a distinct data type from AST, not the same struct with extra fields bolted on.** `ast::Expr` (parser output, untyped) and `tast::TypedExpr` (type-checker output, every node carries a resolved concrete type, every identifier resolved to an id not a string) live in separate modules and are never unified into one "flexible" enum.

2. **Codegen depends only on `tast/`, never on `ast/`.** This is enforced at the module-visibility level (`pub(crate)` boundaries), not left as a convention — codegen must be structurally unable to import raw AST. If codegen needs information it doesn't have, that information is missing from TAST and belongs in the type checker, not inferred ad hoc in codegen.

3. **Effect checking (`require`, `observe`, `reversible`, `@local`/`@wallclock`) runs after typecheck, before lowering** — it consumes TAST and produces a "checked" TAST (or rejects with a diagnostic). It never lives inside the type checker or inside codegen; it is its own phase with its own module, from the version it's introduced (v0.0.6+).

4. Lexer, parser, AST definitions, resolver, type checker, effect checker, and codegen are separate modules from commit #1. No file exceeds ~500 LOC without being split. AST/TAST node definitions contain no logic — only shape.

5. Every phase has its own unit tests, independent of end-to-end tests. TAST fixtures are asserted directly (no LLVM execution needed) separately from codegen fixtures (LLVM IR / execution output). Golden fixture files (`tests/fixtures/*.kai` + `*.expected`) back every language feature.

6. Diagnostics (`{ message, span, severity }`) are a first-class type from v0.0.1, not retrofitted.

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

```kai
let x = 1;          // immutable binding
var y = 1;           // mutable binding, may be reassigned

fn show(s: string) { ... }              // borrowed, immutable — cannot mutate s or its fields
fn touch(mut s: string) { ... }          // borrowed, mutable — may mutate s's contents, still does not own it
```

Rules:
- `let` bindings cannot be reassigned and cannot have their fields mutated through them.
- `var` bindings may be reassigned; if heap-bearing, reassignment releases the old owned value after the replacement is prepared (§9.4).
- Function parameters are borrowed and **immutable by default**. A parameter must be declared `mut` to permit mutation of its contents (`u.name = "x"` inside the function body) — without `mut`, this is a compile error, not a runtime borrow-check.
- `mut` on a parameter changes mutability only. It never changes ownership — a `mut` parameter is still borrowed; the callee never releases it at scope exit, and the caller remains the owner.

This is a hard requirement introduced at v0.0.2 scope (where `let`/`var` first exist) and extended to parameters at v0.0.3 (where function calls with params first exist) — mutability checking is not deferred to a later version, since retrofitting it has the same "touch everything at once" risk called out in §8.

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

This is the rule that v0.4.x got wrong and must be enforced from the version it first becomes reachable (v0.0.3, once `return` combined with heap-bearing params/fields exists):

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

### 9.6 Function calls

Arguments are borrowed by default (see §9.3 for mutability of that borrow). The caller keeps ownership; the callee does not release the argument at scope exit as if it owned the caller's storage.

If a function returns a heap value, ownership transfers to the caller, subject to the retain rule in §9.5 — an owned local moves for free; a borrowed value being returned is retained first.

### 9.7 String literals

String literals are borrowed static storage at expression level. When stored into an owning location, the compiler creates an owned runtime string.

### 9.8 Struct fields

The struct owns heap-bearing fields; releasing the struct releases those fields. Field access (`user.name`) is a borrow. Binding a heap-bearing field (`let name = user.name;`) retains it — `name` becomes a co-owner, independent of `user`'s lifetime.

### 9.9 Array elements

The array owns its elements. Indexing (`arr[0]`) borrows the element for immediate use; binding it retains it. Loop iteration (`for item in arr`) borrows each element per iteration — the array remains the owner of all elements after the loop.

### 9.10 Closure capture

Captured heap values are retained into the closure environment. Releasing the closure releases the environment; the environment destructor decrements its own reference count first, then releases captured heap fields only when the environment reaches zero.

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

The source location is mandatory. Kai's diagnostics (`{ message, span, severity }`) are first-class from v0.0.1 (§8, constraint 6); a panic without a location would be the one error path in the language that doesn't benefit from that investment.

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

- Are `require`, `reversible`, `@local`/`@wallclock` part of the type system (checked, can reject a program) or purely annotations read by tooling? This determines whether an effect-tracking layer is required in the type checker. (Note: regardless of the answer, their runtime failure behavior is now fixed — §10.3, §10.4.)
- Conflict resolution when two overrides on the same field disagree over time — last-write-wins with a flagged history, or hard block until reconciled?
- Where does `observe`'s history report to — local file, opt-in telemetry, or pluggable sink? Needs a decision before v0.0.7.
- Severity heuristics for `kai debt` — fully compiler-inferred, fully config-driven, or hybrid (default + override)? Leaning hybrid; needs a concrete default rule set written down before v0.0.11.
- **Precise definition of "compiler-untraceable boundary"** for §5.1's `@local`→`@wallclock` rule. Queue sends and explicit serialization are clear cases; less clear: does spawning an async task count, does an in-process channel count, does a thread pool hand-off count? Needs an exact, exhaustive list (or a structural rule the compiler can apply generally) before v0.0.6.
- **`@wallclock` serialization format.** The embedded timestamp needs a concrete wire representation once a `Token @wallclock(...)` is serialized (for the queue-send case in §5.1) — needs a decision before v0.0.6, likely tied to whatever `dsl api`/`dsl sql` end up using for payload encoding.
- **Reference cycles.** Pure RC (§9) leaks on cycles — the clearest case is a closure that captures itself (directly or via a chain) for recursion. Not addressed yet; deferred on purpose. Candidate directions to evaluate later: a `weak` reference kind for capture sites the compiler can identify as recursive, or a narrow, opt-in cycle collector scoped only to closure environments (not a general tracing GC, which is explicitly out of scope per §9.12). Needs a decision before recursive closures are considered supported — until then, they are a known-unsound construct and should be flagged, not silently allowed.
- **Decay taxonomy — proposed, not yet adopted.** §5.0 defines Decay as a required field of `Trust⟨C⟩` but does not currently classify *kinds* of Decay; each Trust instance just names its own mechanism in prose. A candidate taxonomy worth evaluating: `temporal` (time passes — Temporal Trust), `external` (an outside authority changes — Contract Trust), `stateful` (in-process world state changes — Correctness Trust), `invalidation` (a specific event revokes the claim — candidate fit for Reversibility, though its Decay is currently left as "—" in §5.0's table and may not need one at all). If this holds up under scrutiny, it sharpens Trust's definition from "a claim with a confidence level" to "a claim with a defined expiration mechanism," which is a stronger and more specific claim than the current draft makes. This is deliberately not promoted into §5.0 yet — it needs to be checked against each of the four instances (including the Transactional/Compensatable split) before it's treated as load-bearing, not just descriptive.