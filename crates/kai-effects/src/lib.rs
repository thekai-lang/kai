//! Effect checker (v0.0.7, §5.1): `escapes-local-context` inference, verified contract `inferred ⊆ declared`,
//! and temporal reachability. Trust<C> lowering for @local/@wallclock → `Trust<C>` is validated here.
//! `require`/`observe` lower into `Trust<C>` locally through this crate too (v0.20 §5.2) — but bypass the
//! call-graph inference subsystem entirely: their violations/records are runtime concerns at the statement's
//! own execution point, with nothing for a caller to propagate (see `trust.rs`, v0.20–v0.22).

pub mod trust;

use kai_diagnostics::{Diagnostic, Span};
use kai_tast::{Effect, EffectSet, KaiType, TemporalOrigin, TypedProgram, TypedStmt};

/// Analyze a typed program (already ownership-resolved) for temporal effects.
/// Mutates `program.fns[].inferred_effects` with least-fixed-point inference,
/// and returns diagnostics (e.g., `inferred ⊄ declared`, `@local` reachability).
pub fn analyze(program: &mut TypedProgram) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // --- 1. Build call graph and direct effects -----------------------------
    // direct_effects[f] = declared_effects if present and contains Escapes, else empty for now
    // For v0.0.7, direct_effects is just the declared set's Escapes (since no primitive queue.send)
    // In future, direct_effects will also include primitive ops with escapes effect.
    let n = program.fns.len();
    let mut direct: Vec<EffectSet> = vec![EffectSet::default(); n];
    for (idx, f) in program.fns.iter().enumerate() {
        if let Some(declared) = &f.declared_effects
            && declared.0.contains(&Effect::EscapesLocalContext) {
                direct[idx].0.push(Effect::EscapesLocalContext);
            }
    }

    // --- 2. Build call graph edges: f -> g if f's body calls g (direct call) or captures closure that may call g ---
    // For v0.0.7, we only handle direct calls (TypedExprKind::Call) and known closure invocations.
    // Closure-as-argument / returned-or-stored and dynamic dispatch are approximated as conservative union later.
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (caller_idx, f) in program.fns.iter().enumerate() {
        let called = collect_called_fn_ids(&f.body);
        for callee_id in called {
            let callee_idx = callee_id as usize;
            if callee_idx < n {
                edges[caller_idx].push(callee_idx);
            }
        }
        // Also handle closures: if f's body contains a closure literal that captures @local and has inferred escapes, that's direct
        // For now, closures are not yet in call graph as separate nodes, but we treat their effects as part of f's direct if they capture @local and f's body may execute them via known closure call.
        // Simplified: if f contains a closure literal with escapes, add its effects to direct
        let closure_effects = collect_closure_effects(&f.body, &program.fns);
        for eff in closure_effects {
            if !direct[caller_idx].0.contains(&eff) {
                direct[caller_idx].0.push(eff);
            }
        }
    }

    // --- 3. Least-fixed-point over SCCs: inferred = direct ∪ ⋃ inferred(callees) ---
    // Initialize inferred = direct
    let mut inferred = direct.clone();
    // Simple iterative fixpoint (n * edges) — sufficient for v0.0.7 small graphs
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..n {
            let mut new_set = inferred[caller].0.clone();
            for &callee in &edges[caller] {
                for eff in &inferred[callee].0 {
                    if !new_set.contains(eff) {
                        new_set.push(eff.clone());
                    }
                }
            }
            if new_set.len() != inferred[caller].0.len() {
                inferred[caller].0 = new_set;
                changed = true;
            }
        }
    }

    // --- 4. Verify declared contract: inferred ⊆ declared (if declared present) ---
    for (idx, f) in program.fns.iter().enumerate() {
        if let Some(declared) = &f.declared_effects {
            for eff in &inferred[idx].0 {
                if !declared.0.contains(eff) {
                    diagnostics.push(Diagnostic::error(
                        format!(
                            "declared effects {} do not cover inferred effects {} (inferred ⊆ declared must hold, §5.1.2)",
                            format_effect_set(declared),
                            format_effect_set(&inferred[idx])
                        ),
                        f.body.stmts.first().map(stmt_span).unwrap_or(Span::new(0, 0)),
                    ).with_file(&f.module));
                    break;
                }
            }
        }
    }

    // --- 5. Write back inferred effects ---
    for (idx, f) in program.fns.iter_mut().enumerate() {
        f.inferred_effects = inferred[idx].clone();
    }

    // --- 6. Reachability invariant: No @local may become reachable from escapes-local-context without @wallclock ---
    // For each call where callee's inferred contains Escapes, check args for @local
    for f in &program.fns {
        check_temporal_reachability(f, &program.fns, &mut diagnostics);
    }

    // --- 7. @wallclock → @local no conversion (v0.0.7: no conversion at all) ---
    // Typecheck already prevents implicit conversion, but we also need to reject any explicit attempt?
    // For v0.0.7, there is no syntax for conversion, so nothing to check. The invariant is that no @wallclock value is used where @local is expected.
    // This is already enforced by type equality (Temporal origin must match). So no extra check here.

    diagnostics
}

fn format_effect_set(set: &EffectSet) -> String {
    if set.0.is_empty() {
        "{}".to_string()
    } else {
        let parts: Vec<String> = set.0.iter().map(|e| format!("{e}")).collect();
        format!("{{{}}}", parts.join(", "))
    }
}

fn stmt_span(stmt: &TypedStmt) -> Span {
    match stmt {
        TypedStmt::Return(v) => v.as_ref().map(|e| e.span).unwrap_or(Span::new(0, 0)),
        TypedStmt::Let(l) => l.init.span,
        TypedStmt::Assign(a) => a.span,
        TypedStmt::If(i) => i.cond.span,
        TypedStmt::While(w) => w.cond.span,
        TypedStmt::For(f) => f.iterable.span,
        TypedStmt::Block(b) => b.stmts.first().map(stmt_span).unwrap_or(Span::new(0, 0)),
        TypedStmt::Require(e) | TypedStmt::Observe(e) | TypedStmt::Expr(e) => e.span,
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => Span::new(0, 0),
    }
}

fn collect_called_fn_ids(block: &kai_tast::TypedBlock) -> Vec<u32> {
    let mut out = Vec::new();
    for stmt in &block.stmts {
        collect_stmt_calls(stmt, &mut out);
    }
    out
}

fn collect_stmt_calls(stmt: &TypedStmt, out: &mut Vec<u32>) {
    match stmt {
        TypedStmt::Return(Some(e)) | TypedStmt::Expr(e) | TypedStmt::Require(e) | TypedStmt::Observe(e) => {
            collect_expr_calls(e, out);
        }
        TypedStmt::Let(l) => collect_expr_calls(&l.init, out),
        TypedStmt::Assign(a) => collect_expr_calls(&a.value, out),
        TypedStmt::If(i) => {
            collect_expr_calls(&i.cond, out);
            for s in &i.then_block.stmts { collect_stmt_calls(s, out); }
            if let Some(b) = &i.else_block { for s in &b.stmts { collect_stmt_calls(s, out); } }
        }
        TypedStmt::For(f) => {
            collect_expr_calls(&f.iterable, out);
            for s in &f.body.stmts { collect_stmt_calls(s, out); }
        }
        TypedStmt::While(w) => {
            collect_expr_calls(&w.cond, out);
            for s in &w.body.stmts { collect_stmt_calls(s, out); }
        }
        TypedStmt::Block(b) => for s in &b.stmts { collect_stmt_calls(s, out); },
        TypedStmt::Return(None) | TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {}
    }
}

fn collect_expr_calls(expr: &kai_tast::TypedExpr, out: &mut Vec<u32>) {
    match &expr.kind {
        kai_tast::TypedExprKind::Call { func, args } => {
            out.push(func.0);
            for a in args { collect_expr_calls(a, out); }
        }
        kai_tast::TypedExprKind::CallIndirect { callee, args } => {
            collect_expr_calls(callee, out);
            for a in args { collect_expr_calls(a, out); }
        }
        kai_tast::TypedExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_calls(lhs, out);
            collect_expr_calls(rhs, out);
        }
        kai_tast::TypedExprKind::FieldAccess { base, .. } => collect_expr_calls(base, out),
        kai_tast::TypedExprKind::Index { base, index } => {
            collect_expr_calls(base, out);
            collect_expr_calls(index, out);
        }
        kai_tast::TypedExprKind::StructLit { values, .. } => for v in values { collect_expr_calls(v, out); },
        kai_tast::TypedExprKind::ArrayLit { elements } => for e in elements { collect_expr_calls(e, out); },
        kai_tast::TypedExprKind::Coalesce { lhs, rhs } => {
            collect_expr_calls(lhs, out);
            collect_expr_calls(rhs, out);
        }
        kai_tast::TypedExprKind::UnwrapOr { receiver, default } => {
            collect_expr_calls(receiver, out);
            collect_expr_calls(default, out);
        }
        kai_tast::TypedExprKind::Catch { base, stmts, tail, .. } => {
            collect_expr_calls(base, out);
            for s in stmts { collect_stmt_calls(s, out); }
            collect_expr_calls(tail, out);
        }
        kai_tast::TypedExprKind::ClosureLit(clo) => {
            for s in &clo.body.stmts { collect_stmt_calls(s, out); }
        }
        kai_tast::TypedExprKind::SomeLit(v) | kai_tast::TypedExprKind::OkLit(v) | kai_tast::TypedExprKind::ErrLit(v) | kai_tast::TypedExprKind::Neg(v) | kai_tast::TypedExprKind::Not(v) | kai_tast::TypedExprKind::Retain(v) => {
            collect_expr_calls(v, out);
        }
        kai_tast::TypedExprKind::LocalRef(_) | kai_tast::TypedExprKind::IntLit(_) | kai_tast::TypedExprKind::FloatLit(_) | kai_tast::TypedExprKind::BoolLit(_) | kai_tast::TypedExprKind::StrLit { .. } | kai_tast::TypedExprKind::NoneLit | kai_tast::TypedExprKind::Invalid => {}
    }
}

fn collect_closure_effects(block: &kai_tast::TypedBlock, program: &[kai_tast::TypedFnDecl]) -> Vec<Effect> {
    // For v0.0.7, closures that capture @local and are inferred to have escapes will be handled via reachability check, not direct.
    // This helper is placeholder for future: if a closure literal in this block has inferred escapes, return it.
    // Since we don't yet have per-closure inferred effects stored, we approximate: if the closure's body contains a call to an escaping function, then it has escapes.
    let mut out = Vec::new();
    for stmt in &block.stmts {
        if let TypedStmt::Let(l) = stmt
            && let kai_tast::TypedExprKind::ClosureLit(clo) = &l.init.kind {
                // Check if closure body calls escaping function
                let called = collect_called_fn_ids(&clo.body);
                for callee_id in called {
                    if let Some(callee) = program.get(callee_id as usize)
                        && (callee.inferred_effects.0.contains(&Effect::EscapesLocalContext) || callee.declared_effects.as_ref().is_some_and(|s| s.0.contains(&Effect::EscapesLocalContext)))
                            && !out.contains(&Effect::EscapesLocalContext) {
                                out.push(Effect::EscapesLocalContext);
                            }
                }
            }
    }
    out
}

fn check_temporal_reachability(f: &kai_tast::TypedFnDecl, all_fns: &[kai_tast::TypedFnDecl], diagnostics: &mut Vec<Diagnostic>) {
    // For each call in f's body where callee has Escapes, check if any arg is @local
    let _inferred = &f.inferred_effects;
    // We need to know callee's inferred effects, not just caller's. So we check per call site.
    check_block_temporal(&f.body, all_fns, diagnostics);
}

fn check_block_temporal(block: &kai_tast::TypedBlock, all_fns: &[kai_tast::TypedFnDecl], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Expr(e) | TypedStmt::Require(e) | TypedStmt::Observe(e) => {
                check_expr_temporal(e, all_fns, diagnostics);
            }
            TypedStmt::Let(l) => check_expr_temporal(&l.init, all_fns, diagnostics),
            TypedStmt::Assign(a) => check_expr_temporal(&a.value, all_fns, diagnostics),
            TypedStmt::If(i) => {
                check_expr_temporal(&i.cond, all_fns, diagnostics);
                check_block_temporal(&i.then_block, all_fns, diagnostics);
                if let Some(b) = &i.else_block { check_block_temporal(b, all_fns, diagnostics); }
            }
            TypedStmt::For(f) => {
                check_expr_temporal(&f.iterable, all_fns, diagnostics);
                check_block_temporal(&f.body, all_fns, diagnostics);
            }
            TypedStmt::Block(b) => check_block_temporal(b, all_fns, diagnostics),
            TypedStmt::Return(Some(e)) => check_expr_temporal(e, all_fns, diagnostics),
            _ => {}
        }
    }
}

fn check_expr_temporal(expr: &kai_tast::TypedExpr, all_fns: &[kai_tast::TypedFnDecl], diagnostics: &mut Vec<Diagnostic>) {
    match &expr.kind {
        kai_tast::TypedExprKind::Call { func, args } => {
            if let Some(callee) = all_fns.get(func.0 as usize) {
                let callee_has_escapes = callee.inferred_effects.0.contains(&Effect::EscapesLocalContext)
                    || callee.declared_effects.as_ref().is_some_and(|s| s.0.contains(&Effect::EscapesLocalContext));
                if callee_has_escapes {
                    for arg in args {
                        if is_local_temporal(&arg.ty) {
                            diagnostics.push(Diagnostic::error(
                                format!("cannot pass `{} @local` to `{} which may cross an `escapes-local-context` boundary without converting to `@wallclock` first (no conversion in v0.0.7, §5.1.4)", arg.ty, callee.name),
                                arg.span,
                            ));
                        }
                        // Also check if arg is closure that captures @local and callee may execute it
                        if let KaiType::Closure { .. } = &arg.ty {
                            // For v0.0.7, conservative: if closure captures @local and callee has escapes, it's reachability violation
                            // We need to know closure's captures - but Call's arg for closure literal would be a closure value, not yet lowered with captures?
                            // For now, placeholder: if arg is closure literal with @local capture, we would have already checked via collect_closure_effects
                        }
                    }
                }
            }
            for a in args { check_expr_temporal(a, all_fns, diagnostics); }
        }
        kai_tast::TypedExprKind::CallIndirect { callee, args } => {
            // Conservative union for dynamic dispatch: treat as may-escapes, check @local args
            // For v0.0.7, any indirect call with @local arg is conservatively flagged if callee type is closure (since we don't know target)
            let is_closure_call = matches!(callee.ty, KaiType::Closure { .. });
            if is_closure_call {
                for arg in args {
                    if is_local_temporal(&arg.ty) {
                        diagnostics.push(Diagnostic::error(
                            format!("cannot pass `{} @local` to indirect closure call which may cross `escapes-local-context` (conservative, §5.1.3)", arg.ty),
                            arg.span,
                        ));
                    }
                }
            }
            check_expr_temporal(callee, all_fns, diagnostics);
            for a in args { check_expr_temporal(a, all_fns, diagnostics); }
        }
        kai_tast::TypedExprKind::ClosureLit(clo) => {
            // Check captures: if closure captures @local and its inferred effects contain Escapes, it's okay only if not stored/called via escaping path?
            // For v0.0.7, we just ensure that if closure captures @local, its own inferred_effects will be checked when it's called/returned/stored.
            // No additional check here beyond what check_block_temporal does for Let that stores closure.
            for _s in &clo.body.stmts {
                // Use a helper to check block for temporal, but we already will via check_block_temporal on the closure body when analyzing the closure as a function?
                // For now, just recurse
                // We need to handle TypedStmt inside closure body - but collect via check_block_temporal on closure body
            }
            // Check if closure is stored or returned where it may reach escapes
            // This is handled at the Let/Return site that has the closure value, not here.
        }
        _ => {
            // Recurse for other expr kinds
            match &expr.kind {
                kai_tast::TypedExprKind::Binary { lhs, rhs, .. } => {
                    check_expr_temporal(lhs, all_fns, diagnostics);
                    check_expr_temporal(rhs, all_fns, diagnostics);
                }
                kai_tast::TypedExprKind::FieldAccess { base, .. } => check_expr_temporal(base, all_fns, diagnostics),
                kai_tast::TypedExprKind::Index { base, index } => {
                    check_expr_temporal(base, all_fns, diagnostics);
                    check_expr_temporal(index, all_fns, diagnostics);
                }
                kai_tast::TypedExprKind::StructLit { values, .. } => for v in values { check_expr_temporal(v, all_fns, diagnostics); },
                kai_tast::TypedExprKind::ArrayLit { elements } => for e in elements { check_expr_temporal(e, all_fns, diagnostics); },
                kai_tast::TypedExprKind::Coalesce { lhs, rhs } => {
                    check_expr_temporal(lhs, all_fns, diagnostics);
                    check_expr_temporal(rhs, all_fns, diagnostics);
                }
                kai_tast::TypedExprKind::UnwrapOr { receiver, default } => {
                    check_expr_temporal(receiver, all_fns, diagnostics);
                    check_expr_temporal(default, all_fns, diagnostics);
                }
                kai_tast::TypedExprKind::Catch { base, stmts, tail, .. } => {
                    check_expr_temporal(base, all_fns, diagnostics);
                    for s in stmts { check_stmt_temporal(s, all_fns, diagnostics); }
                    check_expr_temporal(tail, all_fns, diagnostics);
                }
                kai_tast::TypedExprKind::SomeLit(v) | kai_tast::TypedExprKind::OkLit(v) | kai_tast::TypedExprKind::ErrLit(v) | kai_tast::TypedExprKind::Neg(v) | kai_tast::TypedExprKind::Not(v) | kai_tast::TypedExprKind::Retain(v) => {
                    check_expr_temporal(v, all_fns, diagnostics);
                }
                _ => {}
            }
        }
    }
}

fn check_stmt_temporal(stmt: &TypedStmt, all_fns: &[kai_tast::TypedFnDecl], diagnostics: &mut Vec<Diagnostic>) {
    match stmt {
        TypedStmt::Let(l) => {
            // If let stores a closure that captures @local and the let's ty is closure, and the surrounding function's inferred has escapes?
            // For v0.0.7, the reachability invariant says: No @local may become reachable from a value whose execution may cross escapes without @wallclock.
            // If `l.init` is a closure literal that captures @local, and `l.init.ty` is Closure, and the closure's inferred effects contain Escapes, then storing it in `l.local` where `l`'s type is closure is okay, but if later that closure is passed to escaping call, it will be flagged at call site.
            // For now, no extra check at Let site beyond what check_expr_temporal does for the init.
            check_expr_temporal(&l.init, all_fns, diagnostics);
        }
        _ => check_block_temporal(&kai_tast::TypedBlock { stmts: vec![stmt.clone()] }, all_fns, diagnostics),
    }
}

fn is_local_temporal(ty: &KaiType) -> bool {
    match ty {
        KaiType::Temporal { origin, .. } => *origin == TemporalOrigin::Local,
        KaiType::Optional(inner) => is_local_temporal(inner),
        KaiType::Result { ok, err } => is_local_temporal(ok) || is_local_temporal(err),
        KaiType::Array(inner) => is_local_temporal(inner),
        KaiType::Struct(_) => false, // Structs may contain temporal fields, but for v0.0.7 we treat struct as not temporal unless its field is temporal.
        // For struct, we should check fields via heap bearing, but for now, structs are not considered temporal at top level.
        _ => false,
    }
}
