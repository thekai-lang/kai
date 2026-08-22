//! Expression lowering: untyped `kai-ast` exprs -> typed exprs.

use crate::error;
use kai_ast::{Expr, ExprKind};
use kai_diagnostics::Diagnostic;
use kai_tast::{KaiType, TypedExpr, TypedExprKind};

/// Max value representable by the target type; literals are non-negative in
/// v0.0.1 (no unary minus yet), so one inclusive bound suffices.
fn max_inclusive(ty: KaiType) -> u64 {
    match ty {
        KaiType::Int32 => i32::MAX as u64,
    }
}

pub fn lower(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) -> TypedExpr {
    match &expr.kind {
        ExprKind::IntLit(lit) => {
            let max = max_inclusive(KaiType::Int32);
            if lit.value > max {
                diagnostics.push(error::literal_out_of_range(max, KaiType::Int32, expr.span));
            }
            TypedExpr::new(TypedExprKind::IntLit(lit.value as i32), KaiType::Int32)
        }
    }
}
