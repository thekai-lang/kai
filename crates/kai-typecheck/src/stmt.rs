//! Statement lowering: untyped stmts -> typed stmts, checking return values
//! against the enclosing function's return type.

use crate::error;
use crate::expr;
use kai_ast::{Block, Expr, Stmt, StmtKind};
use kai_diagnostics::Diagnostic;
use kai_tast::{KaiType, TypedBlock, TypedStmt};

pub fn block(block: &Block, return_type: KaiType, diagnostics: &mut Vec<Diagnostic>) -> TypedBlock {
    let mut stmts = Vec::with_capacity(block.stmts.len());
    for stmt in &block.stmts {
        if let Some(lowered) = stmt_inner(stmt, return_type, diagnostics) {
            stmts.push(lowered);
        }
    }
    TypedBlock { stmts }
}

fn stmt_inner(
    stmt: &Stmt,
    return_type: KaiType,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedStmt> {
    match &stmt.kind {
        StmtKind::Return(value) => ret(value.as_ref(), stmt.span, return_type, diagnostics),
    }
}

fn ret(
    value: Option<&Expr>,
    stmt_span: kai_diagnostics::Span,
    return_type: KaiType,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedStmt> {
    match value {
        Some(expr_ast) => {
            let typed = expr::lower(expr_ast, diagnostics);
            if typed.ty != return_type {
                diagnostics.push(error::return_type_mismatch(
                    return_type,
                    typed.ty,
                    expr_ast.span,
                ));
                return None;
            }
            Some(TypedStmt::Return(Some(typed)))
        }
        None => {
            // Bare `return` is only valid in unit functions (none exist in v0.0.1).
            diagnostics.push(error::missing_return_value(return_type, stmt_span));
            None
        }
    }
}
