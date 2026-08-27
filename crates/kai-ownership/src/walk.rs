#[allow(unused_imports)]
use kai_tast::{
    KaiType, LocalId, TypedAssign, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl, TypedFor,
    TypedProgram, TypedStmt, TypedWhile,
};
use super::fresh::FreshIds;
use super::heap::{is_owned_temp, wrap_retain_if_borrowed, HeapBearing};
use super::hoist::hoist_borrow_temps;
use super::scopes::Scopes;

pub(crate) fn push_frame_releases(frame: &[(LocalId, KaiType)], out: &mut Vec<TypedStmt>) {
    for (local, ty) in frame.iter().rev() {
        out.push(TypedStmt::ReleaseLocal {
            local: *local,
            ty: ty.clone(),
        });
    }
}

pub(crate) fn walk_block(
    heap: &HeapBearing,
    mut block: TypedBlock,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> TypedBlock {
    scopes.push();
    let mut out = Vec::with_capacity(block.stmts.len());
    for stmt in std::mem::take(&mut block.stmts) {
        match stmt {
            // Return paths release every live local before leaving — but
            // AFTER the return value exists (it may still read locals it
            // borrows; the §9.5 retain on the value keeps heap content
            // alive past the releases). One node carries both.
            ret @ TypedStmt::Return(_) => {
                let ret = finish_return(heap, ret, scopes, fresh);
                let TypedStmt::Return(value) = ret else {
                    unreachable!("finish_return returns a Return");
                };
                out.push(TypedStmt::ReturnCleanup {
                    value,
                    releases: scopes.releases_all(),
                });
                // The block has terminated: nothing after this executes,
                // and emitting anything past a terminator would produce
                // invalid IR. Remaining source statements are dead code.
                break;
            }
            TypedStmt::ReturnCleanup { .. } => unreachable!("pass-generated node"),
            other => out.extend(walk_stmt(heap, other, scopes, fresh)),
        }
    }
    // Normal block end: this frame's locals, reverse declaration order.
    // Skipped when a return already terminated the block.
    if !matches!(out.last(), Some(TypedStmt::ReturnCleanup { .. })) {
        let frame = scopes.pop();
        push_frame_releases(&frame, &mut out);
    } else {
        scopes.pop();
    }
    block.stmts = out;
    block
}

pub(crate) fn finish_return(
    heap: &HeapBearing,
    ret: TypedStmt,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> TypedStmt {
    let TypedStmt::Return(value) = ret else {
        unreachable!("finish_return on non-return")
    };
    let value = match value {
        Some(mut e) => {
            // Descend first: nested owning slots (struct-literal fields,
            // array-literal elements) need their own markers before we
            // decide whether the result itself needs retaining.
            walk_expr(heap, &mut e, scopes, fresh);
            wrap_retain_if_borrowed(heap, &mut e);
            Some(e)
        }
        None => None,
    };
    TypedStmt::Return(value)
}

pub(crate) fn walk_stmt(
    heap: &HeapBearing,
    stmt: TypedStmt,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> Vec<TypedStmt> {
    match stmt {
        TypedStmt::Let(mut binding) => {
            // Borrow-position temporaries inside the initializer are
            // materialized first (in evaluation order); the root itself is
            // a transfer into the owning slot.
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut binding.init, fresh, scopes, &mut out, true);
            walk_expr(heap, &mut binding.init, scopes, fresh);
            // Owning slot: co-own borrowed sources (§9.4/§9.5 row 3).
            wrap_retain_if_borrowed(heap, &mut binding.init);
            scopes.declare(
                binding.local,
                binding.init.ty.clone(),
                heap.is(&binding.init.ty),
            );
            out.push(TypedStmt::Let(binding));
            out
        }
        TypedStmt::Assign(mut assign) => {
            let mut out = Vec::new();
            // Place steps emit before the value at codegen; hoist their
            // index temporaries in that same order.
            for step in assign.path.iter_mut() {
                if let kai_tast::TypedPlaceStep::Index(idx) = step {
                    hoist_borrow_temps(heap, idx, fresh, scopes, &mut out, false);
                }
            }
            hoist_borrow_temps(heap, &mut assign.value, fresh, scopes, &mut out, true);
            out.push(TypedStmt::Assign(walk_assign(heap, assign, scopes, fresh)));
            out
        }
        TypedStmt::If(mut if_) => {
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut if_.cond, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut if_.cond, scopes, fresh);
            if_.then_block = walk_block(heap, if_.then_block, scopes, fresh);
            if_.else_block =
                if_.else_block.map(|b| walk_block(heap, b, scopes, fresh));
            out.push(TypedStmt::If(if_));
            out
        }
        TypedStmt::For(f) => {
            let (f, pre, end_releases) = walk_for(heap, f, scopes, fresh);
            let mut out = pre;
            out.push(TypedStmt::For(f));
            push_frame_releases(&end_releases, &mut out);
            out
        }
        TypedStmt::While(mut while_) => {
            // v0.0.8.1: condition heap temporaries are hoisted into a
            // loop-owned scope. `cond_prelude` (their bindings) is emitted
            // at the TOP of `while.cond` so the condition evaluates FRESH
            // every iteration; `cond_releases` rides BOTH the back-edge and
            // loop exit — one release per evaluation, never zero.
            scopes.push();
            let mut pre = Vec::new();
            hoist_borrow_temps(heap, &mut while_.cond, fresh, scopes, &mut pre, false);
            walk_expr(heap, &mut while_.cond, scopes, fresh);
            let cond_releases: Vec<(LocalId, KaiType)> = scopes
                .frames
                .last()
                .map(|frame| frame.iter().rev().cloned().collect())
                .unwrap_or_default();
            while_.body = walk_block(heap, while_.body, scopes, fresh);
            let _exit_releases = scopes.pop();
            while_.cond_prelude = pre;
            // Codegen emits cond_releases BOTH on the back-edge and at loop
            // exit — the walker must NOT duplicate them here (double-release
            // corruption found in v0.0.8.1 testing).
            while_.cond_releases = cond_releases;

            let stmts_out = vec![TypedStmt::While(while_)];
            stmts_out
        }
        TypedStmt::Block(block) => {
            vec![TypedStmt::Block(walk_block(heap, block, scopes, fresh))]
        }
        TypedStmt::Require(mut e) => {
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut e, scopes, fresh);
            out.push(TypedStmt::Require(e));
            out
        }
        TypedStmt::Observe(mut e) => {
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut e, scopes, fresh);
            out.push(TypedStmt::Observe(e));
            out
        }
        TypedStmt::Expr(mut e) => {
            let mut out = Vec::new();
            if heap.is(&e.ty) && is_owned_temp(&e) {
                // A heap value computed and thrown away: bind it to a
                // hidden local so scope exit releases it (the statement has
                // no other consumer).
                hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
                return out;
            }
            hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut e, scopes, fresh);
            out.push(TypedStmt::Expr(e));
            out
        }
        // Handled by the caller (return needs surrounding-scope context).
        TypedStmt::Return(_) => unreachable!("returns handled by walk_block"),
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {
            unreachable!("nodes are pass-generated")
        }
    }
}

/// Drains remaining catch-block statements verbatim after an early `return`
/// terminated the block (dead code, kept so spans survive).
pub(crate) fn walk_stmt_rest(rest: &mut Vec<TypedStmt>) -> Vec<TypedStmt> {
    std::mem::take(rest)
}

pub(crate) fn walk_assign(
    heap: &HeapBearing,
    mut assign: TypedAssign,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> TypedAssign {
    walk_expr(heap, &mut assign.value, scopes, fresh);

    assign.release_old = assign.op.is_none() && heap.is(&assign.value.ty);

    // Owning slot: retain borrowed replacements (§9.5). Compound ops exist
    // only on numeric (non-heap) slots in v0.0.5, so this is plain stores
    // only.
    if assign.op.is_none() {
        wrap_retain_if_borrowed(heap, &mut assign.value);
    }
    assign
}

/// Rewrites one `for..in`. An OWNED temporary iterable is materialized into
/// a hidden local declared in the loop's own scope frame: normal completion
/// releases it at `for.end` (the frame pops after the body), and a `return`
/// inside the body releases it through the cleanup chain (B3 fix). The old
/// `iterable_owned` flag path is retired — ownership now rides the same
/// machinery as every local.
///
/// Returns (rewritten loop, statements before it, releases for after it).
pub(crate) fn walk_for(
    heap: &HeapBearing,
    mut f: TypedFor,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> (TypedFor, Vec<TypedStmt>, Vec<(LocalId, KaiType)>) {
    scopes.push();
    let mut pre = Vec::new();
    walk_expr(heap, &mut f.iterable, scopes, fresh);
    if is_owned_temp(&f.iterable) && heap.is(&f.iterable.ty) {
        let local = fresh.alloc();
        let ty = f.iterable.ty.clone();
        let init = std::mem::replace(
            &mut f.iterable,
            TypedExpr::new(TypedExprKind::LocalRef(local), ty.clone()),
        );
        scopes.declare(local, ty, true);
        pre.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$iter".into(),
            init,
        }));
    }
    // The hidden local (or the original owner, for borrowed iterables)
    // carries the release duty; the flag has no further job.
    f.iterable_owned = false;
    f.body = walk_block(heap, f.body, scopes, fresh);
    let frame = scopes.pop();
    (f, pre, frame)
}

pub(crate) fn walk_expr(
    heap: &HeapBearing,
    expr: &mut TypedExpr,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) {
    match &mut expr.kind {
        TypedExprKind::IntLit(_) | TypedExprKind::FloatLit(_) | TypedExprKind::BoolLit(_)
        | TypedExprKind::LocalRef(_) | TypedExprKind::StrLit { .. } | TypedExprKind::Invalid => {}
        TypedExprKind::Neg(inner) | TypedExprKind::Not(inner) | TypedExprKind::Retain(inner) => {
            walk_expr(heap, inner, scopes, fresh)
        }
        TypedExprKind::Binary { op: _, lhs, rhs, .. } => {
            walk_expr(heap, lhs, scopes, fresh);
            walk_expr(heap, rhs, scopes, fresh);
        }
        TypedExprKind::FieldAccess { base, .. } => walk_expr(heap, base, scopes, fresh),
        TypedExprKind::Index { base, index } => {
            walk_expr(heap, base, scopes, fresh);
            walk_expr(heap, index, scopes, fresh);
        }
        // Literal fields/elements ARE owning slots: retain borrowed sources
        // at construction (§9.5 wrap()/pair() examples).
        TypedExprKind::StructLit { values, .. } => {
            for v in values.iter_mut() {
                walk_expr(heap, v, scopes, fresh);
                wrap_retain_if_borrowed(heap, v);
            }
        }
        TypedExprKind::ArrayLit { elements } => {
            for e in elements.iter_mut() {
                walk_expr(heap, e, scopes, fresh);
                wrap_retain_if_borrowed(heap, e);
            }
        }
        // Call arguments are BORROWED (§9.6): no retain, but nested
        // expressions inside argument positions still get walked.
        TypedExprKind::Call { args, .. } => {
            for a in args.iter_mut() {
                walk_expr(heap, a, scopes, fresh);
            }
        }
        // -- v0.0.6 (§9.9a/§9.10) ----------------------------------------
        // `Some(x)` / `Ok(x)` / `Err(x)` build fresh tagged aggregates: payload is an
        // OWNING slot — retain borrowed sources at construction, exactly
        // like struct/array literal members.
        TypedExprKind::SomeLit(value)
        | TypedExprKind::OkLit(value)
        | TypedExprKind::ErrLit(value) => {
            walk_expr(heap, value, scopes, fresh);
            wrap_retain_if_borrowed(heap, value);
        }
        // Carries no payload; nothing to own.
        TypedExprKind::NoneLit => {}
        // Coalesce/UnwrapOr forward whichever branch was active. Consumers
        // treat the result as borrowed (retain-on-bind); when the losing
        // side produced an owned temporary, codegen releases the creator's
        // reference right after the branch join — balanced in both cases.
        // The pass itself adds no markers here.
        TypedExprKind::Coalesce { lhs, rhs }
        | TypedExprKind::UnwrapOr {
            receiver: lhs,
            default: rhs,
        } => {
            walk_expr(heap, lhs, scopes, fresh);
            walk_expr(heap, rhs, scopes, fresh);
        }
        // The catch block owns a scope frame: locals declared inside are
        // released AFTER the tail evaluates (they may feed it). The err
        // binding borrows the Err payload — never tracked.
        TypedExprKind::Catch { base, stmts, tail, releases, .. } => {
            walk_expr(heap, base, scopes, fresh);
            scopes.push();
            let mut done: Vec<TypedStmt> = Vec::with_capacity(stmts.len());
            for s in std::mem::take(stmts) {
                match s {
                    ret @ TypedStmt::Return(_) => {
                        let ret = finish_return(heap, ret, scopes, fresh);
                        let TypedStmt::Return(value) = ret else {
                            unreachable!("finish_return returns a Return");
                        };
                        done.push(TypedStmt::ReturnCleanup {
                            value,
                            releases: scopes.releases_all(),
                        });
                        done.extend(walk_stmt_rest(stmts));
                        break;
                    }
                    other => done.extend(walk_stmt(heap, other, scopes, fresh)),
                }
            }
            *stmts = done;
            if !matches!(stmts.last(), Some(TypedStmt::ReturnCleanup { .. })) {
                *releases = scopes.pop();
            } else {
                scopes.pop();
            }
            walk_expr(heap, tail, scopes, fresh);
        }
        // Arguments borrow as usual; the callee VALUE walks too. Capture
        // retains happen at construction inside codegen (compile-time keyed
        // per capture type), so the pass adds nothing here.
        TypedExprKind::CallIndirect { callee, args } => {
            walk_expr(heap, callee, scopes, fresh);
            for a in args.iter_mut() {
                walk_expr(heap, a, scopes, fresh);
            }
        }
        // The compensation block owns a scope frame: locals declared inside
        // are released after `stmts` evaluate. The base call still borrows
        // its args as usual (§9.6).
        TypedExprKind::Compensate { base, stmts, releases, .. } => {
            walk_expr(heap, base, scopes, fresh);
            scopes.push();
            let mut done: Vec<TypedStmt> = Vec::with_capacity(stmts.len());
            for s in std::mem::take(stmts) {
                match s {
                    ret @ TypedStmt::Return(_) => {
                        let ret = finish_return(heap, ret, scopes, fresh);
                        let TypedStmt::Return(value) = ret else {
                            unreachable!("finish_return returns a Return");
                        };
                        done.push(TypedStmt::ReturnCleanup {
                            value,
                            releases: scopes.releases_all(),
                        });
                        done.extend(walk_stmt_rest(stmts));
                        break;
                    }
                    other => done.extend(walk_stmt(heap, other, scopes, fresh)),
                }
            }
            *stmts = done;
            if !matches!(stmts.last(), Some(TypedStmt::ReturnCleanup { .. })) {
                *releases = scopes.pop();
            } else {
                scopes.pop();
            }
        }
        // A literal is a fresh heap allocation (unconditionally
        // heap-bearing): an owned temp like any StrLit/ArrayLit.
        TypedExprKind::ClosureLit(_) => {}
    }
}
