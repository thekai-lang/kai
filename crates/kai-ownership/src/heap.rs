use kai_tast::{KaiType, TypedExpr, TypedExprKind};

/// Precomputed "does this type own heap memory" table (§9.1 + §9.5):
/// `string` and arrays always; structs iff any field is heap-bearing,
/// recursively. Cycles are impossible — the resolver rejects them, and
/// arrays break any potential cycle by indirection.
pub(crate) struct HeapBearing {
    struct_heap: Vec<bool>,
}

impl HeapBearing {
    pub(crate) fn new(structs: &[kai_tast::TypedStruct]) -> Self {
        // Fixed-point iteration: assume no struct is heap-bearing, then
        // propagate until stable. Forward field references are legal, so
        // one pass is not enough — iterate to a fixpoint.
        let mut heap = Self {
            struct_heap: vec![false; structs.len()],
        };
        loop {
            let mut changed = false;
            for (idx, ts) in structs.iter().enumerate() {
                if heap.struct_heap[idx] {
                    continue;
                }
                if ts.fields.iter().any(|f| heap.is(&f.ty)) {
                    heap.struct_heap[idx] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        heap
    }

    pub(crate) fn is(&self, ty: &KaiType) -> bool {
        match ty {
            KaiType::String | KaiType::Array(_) => true,
            KaiType::Struct(id) => self.struct_heap[id.0 as usize],
            // Tagged unions (§9.9a): heap-bearing iff the ACTIVE payload can
            // be — for Result that means either branch.
            KaiType::Optional(inner) => self.is(inner),
            KaiType::Result { ok, err } => self.is(ok) || self.is(err),
            // Temporal (§5.1): @wallclock is heap-bearing (embedded RFC3339 timestamp), @local is zero-cost.
            KaiType::Temporal { inner, origin, .. } => match origin {
                kai_tast::TemporalOrigin::Wallclock => true,
                kai_tast::TemporalOrigin::Local => self.is(inner),
            },
            // v0.13: closures are unconditionally heap-bearing regardless of
            // capture — mirrors array's rule, never an optimization-driven
            // exception.
            KaiType::Closure { .. } => true,
            KaiType::Int32
            | KaiType::Int64
            | KaiType::Float64
            | KaiType::Bool
            | KaiType::Unit => false,
        }
    }
}

/// Ownership class of an evaluated expression (§9.5 summary table).
pub(crate) fn is_owned_temp(expr: &TypedExpr) -> bool {
    match &expr.kind {
        // Fresh allocations / transferred results.
        TypedExprKind::StrLit { .. }
        | TypedExprKind::ArrayLit { .. }
        | TypedExprKind::StructLit { .. }
        | TypedExprKind::Call { .. }
        // `Some(x)` / `Ok(x)` / `Err(x)` build fresh tagged aggregates (§9.9a/v0.14): payload retained when heap-bearing.
        | TypedExprKind::SomeLit(_)
        | TypedExprKind::OkLit(_)
        | TypedExprKind::ErrLit(_)
        | TypedExprKind::ClosureLit(_) => true,
        // Everything else borrows: bindings, projections, scalars, poison.
        // Coalesce/UnwrapOr/Catch forward a payload chosen at runtime —
        // precise tag-guarded handling lands with the P4 commit; treated as
        // borrows until then.
        _ => false,
    }
}

/// Owning slot fed by a borrowed value (§9.5 row 2): swap the expression
/// for a placeholder, rewrap it in a `Retain` marker that carries the
/// inner span. Shared by every owning-slot site (returns, `let`, plain
/// assignment, literal fields/elements).
pub(crate) fn wrap_retain_if_borrowed(heap: &HeapBearing, e: &mut TypedExpr) {
    // `None` carries no live payload — there is nothing to co-own.
    if matches!(e.kind, TypedExprKind::NoneLit) {
        return;
    }
    if heap.is(&e.ty) && !is_owned_temp(e) {
        let ty = e.ty.clone();
        let span = e.span;
        let inner = std::mem::replace(e, TypedExpr::new(TypedExprKind::Invalid, ty.clone()));
        *e = TypedExpr::new_at(TypedExprKind::Retain(Box::new(inner)), ty, span);
    }
}
