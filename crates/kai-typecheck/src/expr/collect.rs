#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
use super::*;

pub(crate) fn collect_local_refs(block: &kai_tast::TypedBlock, out: &mut Vec<LocalId>) {
    for s in &block.stmts {
        collect_refs_stmt(s, out);
    }
}

pub(crate) fn collect_refs_stmt(s: &kai_tast::TypedStmt, out: &mut Vec<LocalId>) {
    use kai_tast::TypedStmt;
    match s {
        TypedStmt::Let(l) => collect_refs_expr(&l.init, out),
        TypedStmt::Assign(a) => {
            for step in &a.path {
                if let kai_tast::TypedPlaceStep::Index(index) = step {
                    collect_refs_expr(index, out);
                }
            }
            collect_refs_expr(&a.value, out);
        }
        TypedStmt::Return(Some(e)) => collect_refs_expr(e, out),
        TypedStmt::If(i) => {
            collect_refs_expr(&i.cond, out);
            collect_local_refs(&i.then_block, out);
            if let Some(e) = &i.else_block {
                collect_local_refs(e, out);
            }
        }
        TypedStmt::For(f) => {
            collect_refs_expr(&f.iterable, out);
            collect_local_refs(&f.body, out);
        }
        TypedStmt::Block(b) => collect_local_refs(b, out),
        TypedStmt::Require(e) | TypedStmt::Observe(e) | TypedStmt::Expr(e) => collect_refs_expr(e, out),
        // Ownership-pass markers: no user references inside.
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {}
        TypedStmt::Return(None) => {}
    }
}

pub(crate) fn collect_refs_expr(e: &TypedExpr, out: &mut Vec<LocalId>) {
    match &e.kind {
        TypedExprKind::LocalRef(id) => {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        TypedExprKind::Neg(inner)
        | TypedExprKind::Not(inner)
        | TypedExprKind::Retain(inner)
        | TypedExprKind::SomeLit(inner)
        | TypedExprKind::OkLit(inner)
        | TypedExprKind::ErrLit(inner) => collect_refs_expr(inner, out),
        TypedExprKind::Binary { lhs, rhs, .. }
        | TypedExprKind::Coalesce { lhs, rhs } => {
            collect_refs_expr(lhs, out);
            collect_refs_expr(rhs, out);
        }
        TypedExprKind::FieldAccess { base, .. } => collect_refs_expr(base, out),
        TypedExprKind::UnwrapOr { receiver, default } => {
            collect_refs_expr(receiver, out);
            collect_refs_expr(default, out);
        }
        TypedExprKind::Index { base, index } => {
            collect_refs_expr(base, out);
            collect_refs_expr(index, out);
        }
        TypedExprKind::StructLit { values, .. } | TypedExprKind::ArrayLit { elements: values } => {
            for v in values {
                collect_refs_expr(v, out);
            }
        }
        TypedExprKind::Call { args, .. } => {
            for a in args {
                collect_refs_expr(a, out);
            }
        }
        TypedExprKind::CallIndirect { callee, args } => {
            collect_refs_expr(callee, out);
            for a in args {
                collect_refs_expr(a, out);
            }
        }
        TypedExprKind::Catch {
            base, stmts, tail, ..
        } => {
            collect_refs_expr(base, out);
            for s in stmts {
                collect_refs_stmt(s, out);
            }
            collect_refs_expr(tail, out);
        }
        // Nested closures own their captures; literals carry nothing.
        TypedExprKind::ClosureLit(_) | TypedExprKind::NoneLit => {}
        TypedExprKind::IntLit(_) | TypedExprKind::FloatLit(_) | TypedExprKind::BoolLit(_)
        | TypedExprKind::StrLit { .. } | TypedExprKind::Invalid => {}
    }
}

/// True when this expression is a bare identifier naming an IMPORT ALIAS in
/// the current module — such bases go through qualified-call resolution,
/// never the builtin path.

pub(crate) fn is_import_alias(checker: &Checker, base: &Expr) -> bool {
    match &base.kind {
        ExprKind::Ident(ident) => checker.imports().contains_key(&ident.name),
        _ => false,
    }
}

