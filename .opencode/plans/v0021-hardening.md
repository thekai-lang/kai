# v0.0.2.1 Hardening Plan (approved)

Principle: NO language-semantics expansion — pure hardening + doc reconciliation.

## 1. Parser hardening
- **AST** (`kai-ast/src/expr.rs`): add `ExprKind::Invalid` (poisoned recovery node).
- **TAST** (`kai-tast/src/expr.rs`): add `TypedExprKind::Invalid`.
- **Parser** (`kai-parser/src/parser.rs`, `expr.rs`, `stmt.rs`):
  - General recursion budget `expr_budget: u32` checked at `expr()` entry — covers ALL
    recursive productions (parens today; call/postfix when they arrive), not paren-specific.
  - Make unary parsing iterative (collect prefix `-`/`!` ops, build nodes right-to-left).
  - Over-budget: report `"expression nested too deeply"` scoped to the recovery region
    (flag resets after resync, so multiple deep exprs each report), skip balanced tokens
    iteratively, return `ExprKind::Invalid`.
  - Replace ALL placeholder recoveries (`IntLit(0)`, `BoolLit(false)`, invalid-assign-target
    fallback) with `ExprKind::Invalid`.

## 2. Typecheck
- `lower(Invalid)` → defensive diagnostic + `TypedExpr::new(TypedExprKind::Invalid, Int32)`.

## 3. Codegen
- `TypedExprKind::Invalid` → `undef` poison value of the node's type (match stays total).
- Alloca rule stated as its own invariant: **all stack allocations emitted in entry block**
  (`let_stmt` uses temp builder positioned before first entry instruction).
- Invariant test: parse golden IR text; every `alloca` line must be inside `entry:` block.
- Unit-fn coverage: empty body `{}` AND explicit bare `return;` both verify as void fns;
  document empty-body ⇒ `ret void` as designed fallback behavior.

## 4. LocalId hardening
- `kai-typecheck/src/scope.rs`: `declare()` returns
  `DeclareOutcome::{Fresh(LocalInfo), Duplicate(LocalInfo)}` where Duplicate carries the
  ORIGINAL binding info (same id; counter does NOT advance).
- Remove `LocalId(u32::MAX)` hack from stmt.rs binding().
- Test: declare x → id N; declare x again → `Duplicate(info.id == N)`.

## 5. Lexer numeric diagnostics matrix
- Malformed float: digit + `.` + non-digit → specific diagnostic
  `"float literal requires digits after '.'"`, consume the dot, continue.
- Tests: `1.`→diag · `1.2`→ok · `1`→ok · `1.foo`→IntLit+diag+Ident(foo) ·
  `1..2`→explicit outcome · `.5`→explicit rejection.

## 6. Docs reconciliation (docs only)
- EBNF open items: do NOT reopen div-by-zero / int32 overflow (already §10.2: panic).
- Record KNOWN DIVERGENCE: codegen does not yet implement §10.2 panic paths (plain LLVM
  ops); tracked follow-up with version slot TBD.
- float64 IEEE inf/NaN already specified — align wording.
- Only genuinely open: NaN equality semantics.
- Whitepaper changelog: add NON-VERSIONED "Implementation notes" entry documenting
  v0.0.2.1 hardening + explicit versioning scheme (spec = whitepaper v0.3.x,
  compiler = v0.0.y.z).

## Regression tests
- Deep parens (10k+) and deep unary chain (100k `-`) → clean parse-phase Failure, no crash.
- Existing suite stays green; regenerate `tests/fixtures/v0002/main.expected.ll`
  (allocas move to entry block top).

## Verification & ship
- Full workspace test + clippy (0 warnings) + fmt.
- Commit: `fix: v0.0.2.1 hardening — recursion budget, Invalid AST node, entry-block allocas`
- Push origin main.
