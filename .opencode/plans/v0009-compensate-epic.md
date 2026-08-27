# Epic P2: CompensateThunk Semantics & Implementation

**Status:** Locked in Whitepaper (§5.3.6)
**Goal:** Implement true compensation thunks for `compensate` blocks that capture state by-value, execute during panic unwind, and safely release memory during normal commit.

This epic avoids turning `compensate` into a first-class `Closure` value and correctly breaks down the implementation into manageable compiler phases.

## P2.1 — Capture Analysis
- Phase: **Typecheck** (`kai-typecheck/src/expr/tagged.rs` & `collect.rs`)
- Determine the capture set for the `compensate` block (which outer variables are used).
- Validate mutability constraints: Flag compile-time errors if the user attempts to assign to a captured variable inside the `compensate` block (enforcing §5.3.6 `snapshot immutable`).

## P2.2 — Environment Lowering
- Phase: **Typecheck/TAST**
- Generate an internal `CompensateEnv` struct definition for each block.
- Map the captured variables into this struct's fields.

## P2.3 — Ownership (Retain/Release)
- Phase: **Ownership Resolution** (`kai-ownership/src/walk.rs`)
- **At registration:** Insert `Retain` calls for any heap-bearing value packed into the `CompensateEnv`.
- **At unwind/commit:** Provide runtime semantics/dtors to ensure `kai_release` is called on the environment fields when the thunk is destroyed.

## P2.4 — Thunk Generation
- Phase: **Codegen** (`kai-codegen/src/emit/expr/tagged.rs` or similar)
- Emit a private LLVM function (`@_kai_compensate_thunk_N(env_ptr)`).
- Bind the captured variables to `env_ptr` fields and emit the block's `stmts`.

## P2.5 — Runtime Integration
- Phase: **Runtime FFI** (`kai-codegen/src/runtime/reversible.rs`)
- Expand `ReversibleEntry` to hold either a snapshot or a thunk.
- Implement `kai_reversible_push_compensate(fn_ptr, env_ptr)`.
- Update `kai_reversible_unwind` to dispatch thunks.
- Implement cleanup loop at function exit (commit) to release environments.

## P2.6 — Testing
- Add fixtures to `tests/fixtures/reversible/`:
  - `capture_scalar.kai`: verify scalar captures.
  - `capture_string.kai`: verify string capture and release.
  - `capture_heap_object.kai`: verify struct capture by value.
  - `capture_multiple.kai`: verify multiple captures.
  - `capture_after_mutation.kai`: verify `capture-by-value` takes the value at registration time (snapshot), not the final modified value.
  - Check normal commit (no execution, just release).
  - Check panic unwind (execution + release).
  - Verify LIFO order with multiple compensates.
  - Validate ASan leak checks.
