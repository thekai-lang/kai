use kai_tast::{BinaryOp, TypedExpr, TypedExprKind, TypedStmt};

use super::fresh::FreshIds;
use super::heap::{is_owned_temp, HeapBearing};
use super::scopes::Scopes;

/// Recurse into the ALWAYS-evaluated children of `expr` to hoist any
/// owned temporaries sitting in borrow positions. Does NOT hoist
/// `expr` itself — the caller decides whether the root needs materializing.
///
/// Skipped deliberately:
/// - `&&`/`||` rhs subtrees — hoisting would evaluate the rhs temp even
///   when short-circuited; the lhs (always evaluated) IS recursed into.
/// - Struct/array literal members — those are OWNING slots already handled
///   by the retain wrappers; their dtor releases children.
/// - Lazy positions (unwrap_or default, coalesce rhs, catch body/tail,
///   closure literals) — hoisting would change evaluation order; cleanup
///   rides codegen's branch-level claim normalization instead.
pub(crate) fn hoist_children(
    heap: &HeapBearing,
    expr: &mut TypedExpr,
    fresh: &mut FreshIds,
    scopes: &mut Scopes,
    out: &mut Vec<TypedStmt>,
) {
    match &mut expr.kind {
        // Short-circuit: lhs is always evaluated, rhs is lazy (guarded).
        // LHS hoisting is unconditional (pushed to `out`, registered in
        // scope). RHS hoisting goes into `rhs_hoists` WITHOUT scope
        // registration — the codegen emits those Let statements + releases
        // inside the short-circuit rhs basic block, preserving short-circuit
        // semantics (allocation + release only happen when rhs branch taken).
        TypedExprKind::Binary {
            op: BinaryOp::And | BinaryOp::Or,
            lhs,
            rhs,
            rhs_hoists,
        } => {
            hoist_borrow_temps(heap, lhs, fresh, scopes, out, false);
            // Use a throwaway scope for rhs so that nested owned-temp
            // hoists inside the rhs (e.g. string literals in `a == "x"`)
            // are NOT registered in the real scope. If they were, the
            // scope-exit release path would double-free them (once in the
            // short-circuit rhs basic block via rhs_hoists, once at scope
            // exit). The throwaway scope just absorbs the declare() calls
            // so hoist_children doesn't panic on the empty-frames check.
            let mut throwaway = Scopes::default();
            throwaway.push();
            let mut conditional_out = Vec::new();
            hoist_children(heap, rhs, fresh, &mut throwaway, &mut conditional_out);
            hoist_root(heap, rhs, fresh, &mut throwaway, &mut conditional_out, false, false);
            *rhs_hoists = conditional_out;
        }
        // v0.0.6: `Some`/`Ok`/`Err` payloads are owning slots handled by
        // the retain wrapper; recursing into them lets nested owned temps
        // (e.g. a Call inside Ok(Call(...))) be hoisted before the wrapper.
        TypedExprKind::SomeLit(value)
        | TypedExprKind::OkLit(value)
        | TypedExprKind::ErrLit(value) => {
            hoist_borrow_temps(heap, value, fresh, scopes, out, false);
        }
        // v0.0.8.4 (F2/F3): the ALWAYS-evaluated positions (coalesce lhs,
        // unwrap_or receiver, catch base) DO recurse — heap-bearing owned
        // temps inside them were previously never hoisted, leaking one
        // orphan claim per evaluation (t2a/t2b repro).
        TypedExprKind::Coalesce { lhs, .. } => {
            hoist_borrow_temps(heap, lhs, fresh, scopes, out, false);
        }
        TypedExprKind::UnwrapOr { receiver, .. } => {
            hoist_borrow_temps(heap, receiver, fresh, scopes, out, false);
        }
        TypedExprKind::Catch { base, .. } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false);
        }
        TypedExprKind::Compensate { base, .. } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false);
        }
        TypedExprKind::CallIndirect { .. }
        | TypedExprKind::ClosureLit(_)
        | TypedExprKind::NoneLit => {}
        TypedExprKind::Binary { lhs, rhs, .. } => {
            hoist_borrow_temps(heap, lhs, fresh, scopes, out, false);
            hoist_borrow_temps(heap, rhs, fresh, scopes, out, false);
        }
        TypedExprKind::Call { args, .. } => {
            for a in args.iter_mut() {
                hoist_borrow_temps(heap, a, fresh, scopes, out, false);
            }
        }
        TypedExprKind::Index { base, index } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false);
            hoist_borrow_temps(heap, index, fresh, scopes, out, false);
        }
        TypedExprKind::FieldAccess { base, .. } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false)
        }
        // Neg/Not: scalar unary — no heap children to hoist.
        // StructLit/ArrayLit: owning slots — children released by dtor.
        // StrLit/IntLit/FloatLit/BoolLit/LocalRef/Invalid/Retain/NoneLit: leaf nodes.
        _ => {}
    }
}

/// Materializes owned temporaries sitting in BORROW positions into hidden
/// locals declared in the current scope; the ordinary scope machinery then
/// releases them at block exit and on early returns. Without this, values
/// like a string literal passed to a function leak — nobody owns them after
/// the consuming statement ends.
///
/// `root_is_transfer` marks positions whose top node MOVES instead of
/// borrowing (initializer/assignment RHS roots): there the root stays put
/// and only nested borrow positions are rewritten.
///
/// Children are always recursed into FIRST (`hoist_children`), then the
/// root itself is materialized if needed. This ordering ensures that owned
/// temporaries nested inside the root (e.g. a string literal passed as a
/// call argument) are hoisted before the root is replaced with a LocalRef,
/// so every creation claim gets a matching release at scope exit.
pub(crate) fn hoist_borrow_temps(
    heap: &HeapBearing,
    expr: &mut TypedExpr,
    fresh: &mut FreshIds,
    scopes: &mut Scopes,
    out: &mut Vec<TypedStmt>,
    root_is_transfer: bool,
) {
    hoist_children(heap, expr, fresh, scopes, out);
    hoist_root(heap, expr, fresh, scopes, out, root_is_transfer, true);
}

/// Materialize the root node if it is a heap-bearing owned temp.
/// `register_scope` controls whether the local is declared in the
/// enclosing scope (true) or left unregistered (false). The latter is
/// used for rhs of `&&`/`||` where the release must happen inside the
/// short-circuit rhs basic block, not at scope exit.
fn hoist_root(
    heap: &HeapBearing,
    expr: &mut TypedExpr,
    fresh: &mut FreshIds,
    scopes: &mut Scopes,
    out: &mut Vec<TypedStmt>,
    root_is_transfer: bool,
    register_scope: bool,
) {
    if !root_is_transfer && heap.is(&expr.ty) && is_owned_temp(expr) {
        let local = fresh.alloc();
        let ty = expr.ty.clone();
        let init =
            std::mem::replace(expr, TypedExpr::new(TypedExprKind::LocalRef(local), ty.clone()));
        if register_scope {
            scopes.declare(local, ty, true);
        }
        out.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$tmp".into(),
            init,
        }));
    }
}
