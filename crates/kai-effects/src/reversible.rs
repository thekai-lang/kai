use kai_diagnostics::Diagnostic;
use kai_tast::{TypedProgram, TypedStmt};

/// Structural check for `reversible` functions (§5.3, §8 constraint 8).
/// Inside a `reversible` function, every direct call must be either
/// `compensate`-wrapped or target another `reversible` function. This is a
/// validator, not an inferrer — it never emits LedgerPush.
pub(crate) fn check_program(program: &TypedProgram, diagnostics: &mut Vec<Diagnostic>) {
    for f in &program.fns {
        if f.is_reversible {
            check_block(&f.body, &program.fns, diagnostics);
        }
    }
}

fn check_block(
    block: &kai_tast::TypedBlock,
    all_fns: &[kai_tast::TypedFnDecl],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in &block.stmts {
        check_stmt(stmt, all_fns, diagnostics);
    }
}

fn check_stmt(
    stmt: &TypedStmt,
    all_fns: &[kai_tast::TypedFnDecl],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        TypedStmt::Let(l) => check_expr(&l.init, all_fns, false, diagnostics),
        TypedStmt::Assign(a) => {
            for step in &a.path {
                if let kai_tast::TypedPlaceStep::Index(idx) = step {
                    check_expr(idx, all_fns, false, diagnostics);
                }
            }
            check_expr(&a.value, all_fns, false, diagnostics);
        }
        TypedStmt::If(i) => {
            check_expr(&i.cond, all_fns, false, diagnostics);
            check_block(&i.then_block, all_fns, diagnostics);
            if let Some(b) = &i.else_block {
                check_block(b, all_fns, diagnostics);
            }
        }
        TypedStmt::For(f) => {
            check_expr(&f.iterable, all_fns, false, diagnostics);
            check_block(&f.body, all_fns, diagnostics);
        }
        TypedStmt::While(w) => {
            for s in &w.cond_prelude {
                check_stmt(s, all_fns, diagnostics);
            }
            check_expr(&w.cond, all_fns, false, diagnostics);
            check_block(&w.body, all_fns, diagnostics);
        }
        TypedStmt::Block(b) => check_block(b, all_fns, diagnostics),
        TypedStmt::Return(Some(e))
        | TypedStmt::Expr(e)
        | TypedStmt::Require(e)
        | TypedStmt::Observe(e) => check_expr(e, all_fns, false, diagnostics),
        TypedStmt::Return(None) | TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {}
    }
}

fn check_expr(
    expr: &kai_tast::TypedExpr,
    all_fns: &[kai_tast::TypedFnDecl],
    inside_compensate_base: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        kai_tast::TypedExprKind::Call { func, args } => {
            let callee_is_reversible = all_fns
                .get(func.0 as usize)
                .is_some_and(|f| f.is_reversible);
            if !inside_compensate_base && !callee_is_reversible {
                let callee_name = all_fns
                    .get(func.0 as usize)
                    .map(|f| f.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(
                    format!(
                        "call to `{callee_name}` inside `reversible` function must be wrapped in `compensate` or target must be `reversible` (§5.3)"
                    ),
                    expr.span,
                ));
            }
            for a in args {
                check_expr(a, all_fns, false, diagnostics);
            }
        }
        kai_tast::TypedExprKind::CallIndirect { callee, args } => {
            if !inside_compensate_base {
                diagnostics.push(Diagnostic::error(
                    "indirect closure call inside `reversible` function must be wrapped in `compensate` (§5.3, indirect calls are not `reversible`)",
                    expr.span,
                ));
            }
            check_expr(callee, all_fns, false, diagnostics);
            for a in args {
                check_expr(a, all_fns, false, diagnostics);
            }
        }
        kai_tast::TypedExprKind::Compensate { base, .. } => {
            // The compensation block executes on unwind, not as part of the
            // reversible forward path; its internal calls are not subject to
            // the `reversible` wrapper rule (§5.3 deems compensate stmts
            // as the compensation actions themselves).
            check_expr(base, all_fns, true, diagnostics);
        }
        kai_tast::TypedExprKind::Binary { lhs, rhs, .. } => {
            check_expr(lhs, all_fns, false, diagnostics);
            check_expr(rhs, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::FieldAccess { base, .. } => {
            check_expr(base, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::Index { base, index } => {
            check_expr(base, all_fns, false, diagnostics);
            check_expr(index, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::StructLit { values, .. } => {
            for v in values {
                check_expr(v, all_fns, false, diagnostics);
            }
        }
        kai_tast::TypedExprKind::ArrayLit { elements } => {
            for e in elements {
                check_expr(e, all_fns, false, diagnostics);
            }
        }
        kai_tast::TypedExprKind::Coalesce { lhs, rhs } => {
            check_expr(lhs, all_fns, false, diagnostics);
            check_expr(rhs, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::UnwrapOr { receiver, default } => {
            check_expr(receiver, all_fns, false, diagnostics);
            check_expr(default, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::Catch { base, stmts, tail, .. } => {
            check_expr(base, all_fns, false, diagnostics);
            for s in stmts {
                check_stmt(s, all_fns, diagnostics);
            }
            check_expr(tail, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::ClosureLit(_) => {
            diagnostics.push(Diagnostic::error(
                "closures inside `reversible` functions are not supported (they cannot securely carry transactional effects into deferred execution)",
                expr.span,
            ));

            // Closures do not inherit the `reversible` context.
            // They execute on their own, outside the transactional activation.
            // Note: Since closures in v0.0.9 cannot be `reversible` themselves, they cannot contain transactional effects.
        }
        kai_tast::TypedExprKind::SomeLit(v)
        | kai_tast::TypedExprKind::OkLit(v)
        | kai_tast::TypedExprKind::ErrLit(v)
        | kai_tast::TypedExprKind::Neg(v)
        | kai_tast::TypedExprKind::Not(v)
        | kai_tast::TypedExprKind::Retain(v) => {
            check_expr(v, all_fns, false, diagnostics);
        }
        kai_tast::TypedExprKind::LocalRef(_)
        | kai_tast::TypedExprKind::IntLit(_)
        | kai_tast::TypedExprKind::FloatLit(_)
        | kai_tast::TypedExprKind::BoolLit(_)
        | kai_tast::TypedExprKind::StrLit { .. }
        | kai_tast::TypedExprKind::NoneLit
        | kai_tast::TypedExprKind::Invalid => {}
    }
}
