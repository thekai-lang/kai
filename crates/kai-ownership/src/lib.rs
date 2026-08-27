//! Ownership resolution (§9): the explicit IR-producing phase between
//! typecheck and codegen. Every retain/release/move decision is materialized
//! as a TAST node here — `TypedExprKind::Retain`, `TypedStmt::ReleaseLocal`,
//! `TypedAssign::release_old` — so codegen reads mechanically and never
//! infers ownership itself (§8, constraint 2).
//!
//! The model (§9.4–9.9):
//! - Locals (`let`/`var`) OWN their slot contents; parameters BORROW.
//! - Reading any binding yields a borrowed reference (the source keeps
//!   ownership); only fresh allocations are owned temporaries: string
//!   literals, array literals, struct literals, call results.
//! - Entering an OWNING SLOT with a borrowed value inserts a retain
//!   (co-ownership). Owning slots: `let`/`var` inits, assignment targets,
//!   return values of heap-typed functions, struct-literal fields, array-
//!   literal elements.
//! - Replacing an owning slot's content releases the OLD value after the
//!   replacement is prepared (§9.4 ordering, E4).
//! - Scope exit releases locals innermost-first, reverse declaration order;
//!   return paths release every enclosing frame before leaving.
//! - Owned temporaries in BORROW positions (call arguments, comparison and
//!   projection operands, discarded statement values) are materialized into
//!   hidden `$tmp` locals so scope machinery releases them; without this
//!   they would leak. `&&`/`||` subtrees are exempt — hoisting would defeat
//!   short-circuiting.
//! - `for..in` over an owned temporary binds it to a hidden `$iter` local in
//!   the loop's scope frame: released at loop exit AND on returns from the
//!   body (the old flag-based path could leak on early return). The loop
//!   variable never owns (E7).

mod fresh;
mod heap;
mod hoist;
mod scopes;
mod walk;

#[allow(unused_imports)]
use kai_tast::{BinaryOp, KaiType, LocalId, TypedAssign, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl, TypedFor, TypedProgram, TypedStmt};
use fresh::FreshIds;
use heap::HeapBearing;
use scopes::Scopes;
use walk::walk_block;
#[allow(unused_imports)]
pub(crate) use hoist::hoist_borrow_temps;
#[allow(unused_imports)]
pub(crate) use walk::{
    finish_return, push_frame_releases, walk_assign, walk_expr, walk_for, walk_stmt, walk_stmt_rest,
};

/// Runs the pass over a typechecked program, annotating it in place.
pub fn resolve(program: &mut TypedProgram) {
    let heap = HeapBearing::new(&program.structs);
    let mut fresh = FreshIds::seeded_beyond(program);
    for fns in &mut program.fns {
        resolve_fn(&heap, fns, &mut fresh);
    }
}

fn resolve_fn(heap: &HeapBearing, decl: &mut kai_tast::TypedFnDecl, fresh: &mut FreshIds) {
    // Frame 0: parameters — they borrow, so they are never registered for
    // release (the callee does not release what it does not own, §9.3).
    let mut scopes = Scopes::default();
    scopes.push();
    for param in &decl.params {
        scopes.declare(param.local, param.ty.clone(), false);
    }
    let body = std::mem::replace(&mut decl.body, TypedBlock { stmts: Vec::new() });
    decl.body = walk_block(heap, body, &mut scopes, fresh);
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod scope_tests;
#[cfg(test)]
mod v0006_tests;
