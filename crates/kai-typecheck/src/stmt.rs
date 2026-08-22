//! Statement typing: bindings (`let`/`var`), assignment with mutability
//! enforcement, if/else with bool conditions, return, expression statements.

use crate::checker::Checker;
use crate::error;
use crate::expr;
use crate::ty;
use kai_ast::{AssignOp, Block as AstBlock, Stmt, StmtKind};
use kai_tast::{
    BinaryOp, KaiType, TypedAssign, TypedBlock, TypedExpr, TypedIf, TypedLet, TypedStmt,
};

/// Lower a whole function body. `return_type` drives return checks.
pub(crate) fn lower_block(
    checker: &mut Checker,
    block: &AstBlock,
    return_type: KaiType,
) -> TypedBlock {
    checker.locals.push_scope();
    let stmts = block
        .stmts
        .iter()
        .filter_map(|s| lower_stmt(checker, s, return_type))
        .collect();
    checker.locals.pop_scope();
    TypedBlock { stmts }
}

fn lower_stmt(checker: &mut Checker, stmt: &Stmt, return_type: KaiType) -> Option<TypedStmt> {
    match &stmt.kind {
        StmtKind::Return(value) => ret(checker, value.as_ref(), return_type, stmt.span),
        StmtKind::Let(b) => binding(checker, b.name.clone(), b.mutable, b.ty.as_ref(), &b.init),
        StmtKind::Assign(a) => assign(checker, &a.target, a.op, &a.value),
        StmtKind::If(i) => if_stmt(checker, i, return_type),
        StmtKind::Block(b) => Some(TypedStmt::Block(lower_block(checker, b, return_type))),
        StmtKind::Expr(e) => Some(TypedStmt::Expr(expr::lower(checker, e, None))),
    }
}

fn ret(
    checker: &mut Checker,
    value: Option<&kai_ast::Expr>,
    return_type: KaiType,
    span: kai_diagnostics::Span,
) -> Option<TypedStmt> {
    let value = value.map(|e| expr::lower(checker, e, Some(return_type)));

    match &value {
        None => {
            if return_type != KaiType::Unit {
                checker.error(error::missing_return_value(return_type, span));
            }
        }
        Some(v) if v.ty != return_type => {
            let found = v.ty;
            checker.error(error::return_type_mismatch(return_type, found, span));
        }
        Some(_) => {}
    }

    Some(TypedStmt::Return(value))
}

#[allow(clippy::too_many_arguments)]
fn binding(
    checker: &mut Checker,
    name: kai_ast::Ident,
    mutable: bool,
    annotation: Option<&kai_ast::Ty>,
    init: &kai_ast::Expr,
) -> Option<TypedStmt> {
    let annotated = annotation.map(|ty| ty::resolve(checker, ty));
    let value = expr::lower(checker, init, annotated);

    if let Some(expected) = annotated
        && expected != value.ty
    {
        let found = value.ty;
        let name_text = name.name.clone();
        checker.error(error::init_type_mismatch(
            &name_text, expected, found, init.span,
        ));
    }

    if value.ty == KaiType::Unit && annotated.is_none() {
        checker.error(error::unit_binding(init.span));
    }

    let declared_name = name.name.clone();
    let span = name.span;
    match checker.locals.declare(&declared_name, value.ty, mutable) {
        Some(info) => Some(TypedStmt::Let(TypedLet {
            local: info.id,
            name: declared_name,
            init: value,
        })),
        None => {
            // Redeclaration: keep a fresh id so codegen stays consistent.
            let id = kai_tast::LocalId(u32::MAX);
            checker.error(error::duplicate_local(&declared_name, span));
            Some(TypedStmt::Let(TypedLet {
                local: id,
                name: declared_name,
                init: value,
            }))
        }
    }
}

fn assign(
    checker: &mut Checker,
    target: &kai_ast::AssignTarget,
    op: AssignOp,
    value: &kai_ast::Expr,
) -> Option<TypedStmt> {
    let kai_ast::AssignTarget::Named(name) = target;

    let info = match checker.locals.lookup(&name.name) {
        Some(info) => info,
        None => {
            let span = name.span;
            let text = name.name.clone();
            checker.error(error::undeclared_variable(&text, span));
            return None;
        }
    };

    if !info.mutable {
        let span = name.span;
        let text = name.name.clone();
        checker.error(error::assign_to_immutable(&text, span));
    }

    let typed_value = expr::lower(checker, value, Some(info.ty));

    // Compound ops are read-modify-write: validate like the binary operator.
    let compound = compound_op(op);
    match compound {
        Some(binop) => {
            check_compound(checker, binop, info.ty, &typed_value, value.span);
        }
        None if typed_value.ty != info.ty => {
            let span = value.span;
            let found = typed_value.ty;
            let text = name.name.clone();
            checker.error(kai_diagnostics::Diagnostic::error(
                format!("cannot assign `{found}` to `{text}` of type `{}`", info.ty),
                span,
            ));
        }
        None => {}
    }

    Some(TypedStmt::Assign(TypedAssign {
        local: info.id,
        op: compound,
        value: typed_value,
    }))
}

fn check_compound(
    checker: &mut Checker,
    op: BinaryOp,
    target_ty: KaiType,
    value: &TypedExpr,
    span: kai_diagnostics::Span,
) {
    let ok = match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            target_ty == value.ty
                && target_ty.is_numeric()
                && (op != BinaryOp::Mod || target_ty.is_integer())
        }
        _ => false,
    };
    if !ok {
        let name = op.describe();
        checker.error(error::binary_type_mismatch(name, target_ty, value.ty, span));
    }
}

fn compound_op(op: AssignOp) -> Option<BinaryOp> {
    match op {
        AssignOp::Eq => None,
        AssignOp::PlusEq => Some(BinaryOp::Add),
        AssignOp::MinusEq => Some(BinaryOp::Sub),
        AssignOp::StarEq => Some(BinaryOp::Mul),
        AssignOp::SlashEq => Some(BinaryOp::Div),
    }
}

fn if_stmt(
    checker: &mut Checker,
    if_: &kai_ast::IfStmt,
    return_type: KaiType,
) -> Option<TypedStmt> {
    let cond = expr::lower(checker, &if_.cond, None);
    if cond.ty != KaiType::Bool {
        let span = if_.cond.span;
        let found = cond.ty;
        checker.error(error::condition_not_bool(found, span));
    }

    // Branches get their own scopes; returns inside are checked against the
    // enclosing function's return type.
    let then_block = lower_block(checker, &if_.then_block, return_type);
    let else_block = if_
        .else_block
        .as_ref()
        .map(|b| lower_block(checker, b, return_type));

    Some(TypedStmt::If(TypedIf {
        cond,
        then_block,
        else_block,
    }))
}
