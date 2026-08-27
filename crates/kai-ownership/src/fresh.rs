use kai_tast::{TypedExpr, TypedExprKind, TypedStmt};
use kai_tast::LocalId;

/// Allocates local ids beyond everything present in the source tree, used
/// for hidden locals introduced by this pass (hoisted temporaries, owned
/// `for` iterables).
pub(crate) struct FreshIds {
    pub(crate) next: u32,
}

impl Default for FreshIds {
    fn default() -> Self {
        Self { next: 10_000 }
    }
}

impl FreshIds {
    pub(crate) fn seeded_beyond(program: &kai_tast::TypedProgram) -> Self {
        let mut max = 0;
        for decl in &program.fns {
            for p in &decl.params {
                max = max.max(p.local.0);
            }
            seed_stmts(&decl.body.stmts, &mut max);
        }
        Self { next: max + 1 }
    }

    pub(crate) fn alloc(&mut self) -> LocalId {
        let id = LocalId(self.next);
        self.next += 1;
        id
    }
}

fn seed_stmts(stmts: &[TypedStmt], max: &mut u32) {
    for s in stmts {
        match s {
            TypedStmt::Let(b) => {
                *max = (*max).max(b.local.0);
                seed_expr(&b.init, max);
            }
            TypedStmt::Assign(a) => {
                *max = (*max).max(a.root.0);
                for step in &a.path {
                    if let kai_tast::TypedPlaceStep::Index(idx) = step {
                        seed_expr(idx, max);
                    }
                }
                seed_expr(&a.value, max);
            }
            TypedStmt::If(i) => {
                seed_expr(&i.cond, max);
                seed_stmts(&i.then_block.stmts, max);
                if let Some(b) = &i.else_block {
                    seed_stmts(&b.stmts, max);
                }
            }
            TypedStmt::For(f) => {
                *max = (*max).max(f.binding_local.0);
                seed_expr(&f.iterable, max);
                seed_stmts(&f.body.stmts, max);
            }
            TypedStmt::While(w) => {
                seed_expr(&w.cond, max);
                seed_stmts(&w.body.stmts, max);
            }
            TypedStmt::Block(b) => seed_stmts(&b.stmts, max),
            TypedStmt::Expr(e)
            | TypedStmt::Require(e)
            | TypedStmt::Observe(e)
            | TypedStmt::Return(Some(e)) => seed_expr(e, max),
            TypedStmt::Return(None)
            | TypedStmt::ReleaseLocal { .. }
            | TypedStmt::ReturnCleanup { .. } => {}
        }
    }
}

fn seed_expr(expr: &TypedExpr, max: &mut u32) {
    match &expr.kind {
        TypedExprKind::LocalRef(id) => *max = (*max).max(id.0),
        TypedExprKind::Neg(inner) | TypedExprKind::Not(inner) | TypedExprKind::Retain(inner) => {
            seed_expr(inner, max)
        }
        TypedExprKind::Binary { lhs, rhs, .. } => {
            seed_expr(lhs, max);
            seed_expr(rhs, max);
        }
        TypedExprKind::FieldAccess { base, .. } => seed_expr(base, max),
        TypedExprKind::Index { base, index } => {
            seed_expr(base, max);
            seed_expr(index, max);
        }
        TypedExprKind::StructLit { values, .. } | TypedExprKind::ArrayLit { elements: values } => {
            for v in values {
                seed_expr(v, max);
            }
        }
        TypedExprKind::Call { args, .. } => {
            for a in args {
                seed_expr(a, max);
            }
        }
        TypedExprKind::CallIndirect { callee, args } => {
            seed_expr(callee, max);
            for a in args {
                seed_expr(a, max);
            }
        }
        // v0.0.6: children may reference locals; the aggregate nodes
        // themselves carry no LocalRef.
        TypedExprKind::SomeLit(inner)
        | TypedExprKind::OkLit(inner)
        | TypedExprKind::ErrLit(inner) => seed_expr(inner, max),
        TypedExprKind::Coalesce { lhs, rhs }
        | TypedExprKind::UnwrapOr {
            receiver: lhs,
            default: rhs,
        } => {
            seed_expr(lhs, max);
            seed_expr(rhs, max);
        }
        TypedExprKind::Catch { base, stmts, tail, .. } => {
            seed_expr(base, max);
            for s in stmts {
                seed_stmts(std::slice::from_ref(s), max);
            }
            seed_expr(tail, max);
        }
        TypedExprKind::Compensate { base, stmts, .. } => {
            seed_expr(base, max);
            for s in stmts {
                seed_stmts(std::slice::from_ref(s), max);
            }
        }
        // Closure bodies/captures are scoped to the literal; their ids were
        // seeded while walking it (captures reference pre-existing locals).
        TypedExprKind::ClosureLit(clo) => {
            for cap in &clo.captures {
                *max = (*max).max(cap.local.0);
            }
        }
        TypedExprKind::IntLit(_)
        | TypedExprKind::FloatLit(_)
        | TypedExprKind::BoolLit(_)
        | TypedExprKind::NoneLit
        | TypedExprKind::StrLit { .. }
        | TypedExprKind::Invalid => {}
    }
}
