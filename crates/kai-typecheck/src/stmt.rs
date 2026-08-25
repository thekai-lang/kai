//! Statement typing: bindings (`let`/`var`), assignment with mutability
//! enforcement, if/else with bool conditions, return, expression statements.

use crate::checker::Checker;
use crate::error;
use crate::expr;
use crate::ty;
use kai_ast::{AssignOp, Block as AstBlock, Stmt, StmtKind};
use kai_tast::{
    BinaryOp, FieldStep, KaiType, TypedAssign, TypedBlock, TypedExpr, TypedFor, TypedIf,
    TypedLet, TypedPlaceStep, TypedStmt,
};

/// Lower a whole function body. `return_type` drives return checks.
pub(crate) fn lower_block(
    checker: &mut Checker,
    block: &AstBlock,
    return_type: &KaiType,
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

pub(crate) fn lower_stmt(
    checker: &mut Checker,
    stmt: &Stmt,
    return_type: &KaiType,
) -> Option<TypedStmt> {
    match &stmt.kind {
        StmtKind::Return(value) => ret(checker, value.as_ref(), return_type, stmt.span),
        StmtKind::Let(b) => binding(checker, b.name.clone(), b.mutable, b.ty.as_ref(), &b.init),
        StmtKind::Assign(a) => assign(checker, &a.target, a.op, &a.value, a.span),
        StmtKind::If(i) => if_stmt(checker, i, return_type),
        StmtKind::Block(b) => Some(TypedStmt::Block(lower_block(checker, b, return_type))),
        // `_ = expr;` (§9.9b): the expression evaluates under ordinary
        // rules; only the binding is skipped. Interim P1 form — the
        // Optional/Result discard diagnostic joins in P3.
        StmtKind::Discard(value) => Some(TypedStmt::Expr(expr::lower(checker, value, None))),
        StmtKind::Require(expr) => {
            let typed = expr::lower(checker, expr, None);
            if typed.ty != KaiType::Bool {
                checker.error(error::condition_not_bool(typed.ty.clone(), expr.span));
            }
            // Parsed in v0.0.7, semantics not yet formalized (§5.2, v0.0.8). Diagnostic instead of silent.
            checker.error(error::require_not_yet_implemented(stmt.span));
            Some(TypedStmt::Require(typed))
        }
        StmtKind::Observe(expr) => {
            let typed = expr::lower(checker, expr, None);
            if typed.ty != KaiType::Bool {
                checker.error(error::condition_not_bool(typed.ty.clone(), expr.span));
            }
            checker.error(error::observe_not_yet_implemented(stmt.span));
            Some(TypedStmt::Observe(typed))
        }
        StmtKind::Expr(e) => {
            let typed = expr::lower(checker, e, None);
            // §9.9a (v0.0.6): discarding an Optional/Result as a bare
            // statement is a diagnostic — symmetrically for both. The sole
            // escape hatch is the Discard statement below.
            if typed.ty.is_tagged_union() {
                checker.error(error::discard_tagged(&typed.ty, e.span));
            }
            Some(TypedStmt::Expr(typed))
        },
        StmtKind::For(f) => for_stmt(checker, f),
    }
}

fn ret(
    checker: &mut Checker,
    value: Option<&kai_ast::Expr>,
    return_type: &KaiType,
    span: kai_diagnostics::Span,
) -> Option<TypedStmt> {
    let value = value.map(|e| expr::lower(checker, e, Some(return_type.clone())));

    match &value {
        None => {
            if *return_type != KaiType::Unit {
                checker.error(error::missing_return_value(return_type.clone(), span));
            }
        }
        Some(v) if v.ty != *return_type => {
            let found = v.ty.clone();
            checker.error(error::return_type_mismatch(return_type.clone(), found, span));
        }
        Some(_) => {}
    }

    Some(TypedStmt::Return(value))
}

fn binding(
    checker: &mut Checker,
    name: kai_ast::Ident,
    mutable: bool,
    annotation: Option<&kai_ast::Ty>,
    init: &kai_ast::Expr,
) -> Option<TypedStmt> {
    let annotated = annotation.map(|ty| ty::resolve(checker, ty));
    let value = expr::lower(checker, init, annotated.clone());

    if let Some(expected) = &annotated
        && *expected != value.ty
    {
        let found = value.ty.clone();
        let name_text = name.name.clone();
        checker.error(error::init_type_mismatch(
            &name_text,
            expected.clone(),
            found,
            init.span,
        ));
    }

    if value.ty == KaiType::Unit && annotated.is_none() {
        checker.error(error::unit_binding(init.span));
    }

    let declared_name = name.name.clone();
    let span = name.span;
    let declared_ty = value.ty.clone();
    match checker.locals.declare(&declared_name, declared_ty, mutable) {
        crate::scope::DeclareOutcome::Fresh(info) => Some(TypedStmt::Let(TypedLet {
            local: info.id,
            name: declared_name,
            init: value,
        })),
        // Redeclaration keeps the ORIGINAL id so references resolve to the
        // first binding; the diagnostic alone flags the error.
        crate::scope::DeclareOutcome::Duplicate(info) => {
            checker.error(error::duplicate_local(&declared_name, span));
            Some(TypedStmt::Let(TypedLet {
                local: info.id,
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
    stmt_span: kai_diagnostics::Span,
) -> Option<TypedStmt> {
    // The ROOT binding gates the whole place (§9.3): `mut` on a stack-type
    // param is a purely local permission; immutability of the root rejects
    // writes through any field path too.
    let (root_name, root_span, place_steps) = match target {
        kai_ast::AssignTarget::Named(name) => (name.name.clone(), name.span, Vec::new()),
        kai_ast::AssignTarget::Path { root, steps } => (root.name.clone(), root.span, steps.clone()),
    };

    let info = match checker.locals.lookup(&root_name) {
        Some(info) => info,
        None => {
            checker.error(error::undeclared_variable(&root_name, root_span));
            return None;
        }
    };

    if !info.mutable {
        checker.error(error::assign_to_immutable(&root_name, root_span));
    }

    // Walk the projection chain through the SAME hop rules expression
    // indexing/field access uses. The index EXPRESSION is re-evaluated at
    // every assignment site — it rides in the TAST step (§9.3).
    let mut cur_ty = info.ty.clone();
    let mut steps: Vec<TypedPlaceStep> = Vec::new();
    for step in &place_steps {
        match step {
            kai_ast::PlaceStep::Field(seg) => {
                let (struct_id, field, ty) =
                    expr::resolve_field_hop(checker, &cur_ty, &seg.name, seg.span)?;
                steps.push(TypedPlaceStep::Field(FieldStep { struct_id, field }));
                cur_ty = ty;
            }
            kai_ast::PlaceStep::Index { index, rbracket } => {
                let (elem_ty, typed_index) =
                    expr::resolve_index_hop(checker, &cur_ty, index, *rbracket)?;
                steps.push(TypedPlaceStep::Index(Box::new(typed_index)));
                cur_ty = elem_ty;
            }
        }
    }

    let typed_value = expr::lower(checker, value, Some(cur_ty.clone()));

    // Compound ops are read-modify-write: validate like the binary operator.
    let compound = compound_op(op);
    match compound {
        Some(binop) => {
            check_compound(checker, binop, &cur_ty, &typed_value, value.span);
        }
        None if typed_value.ty != cur_ty => {
            let span = value.span;
            let found = typed_value.ty.clone();
            checker.error(error::assign_type_mismatch(&cur_ty, found, span));
        }
        None => {}
    }

    Some(TypedStmt::Assign(TypedAssign {
        root: info.id,
        path: steps,
        op: compound,
        value: typed_value,
        // Ownership markers land in the ownership pass (phase after this).
        release_old: false,
        span: stmt_span,
    }))
}

fn check_compound(
    checker: &mut Checker,
    op: BinaryOp,
    target_ty: &KaiType,
    value: &TypedExpr,
    span: kai_diagnostics::Span,
) {
    let ok = match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            *target_ty == value.ty
                && target_ty.is_numeric()
                && (op != BinaryOp::Mod || target_ty.is_integer())
        }
        _ => false,
    };
    if !ok {
        let name = op.describe();
        checker.error(error::binary_type_mismatch(name, target_ty.clone(), value.ty.clone(), span));
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
    return_type: &KaiType,
) -> Option<TypedStmt> {
    let cond = expr::lower(checker, &if_.cond, None);
    if cond.ty != KaiType::Bool {
        let span = if_.cond.span;
        let found = cond.ty.clone();
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

// -- v0.0.5: for..in ----------------------------------------------------------

/// `for name in array { body }`: the iterable must be `T[]`; `name` becomes
/// an IMMUTABLE element-typed local inside the loop's own scope (§9.9). The
/// borrow-not-own behavior is the ownership pass's concern; here we only
/// fix shapes.
fn for_stmt(checker: &mut Checker, f: &kai_ast::ForStmt) -> Option<TypedStmt> {
    let iterable = expr::lower(checker, &f.iterable, None);

    let elem_ty = match iterable.ty.clone() {
        KaiType::Array(elem) => *elem,
        other => {
            checker.error(error::for_iterable_not_array(&other, f.iterable.span));
            return None;
        }
    };

    checker.locals.push_scope();
    let declared_name = f.binding.name.clone();
    let binding_span = f.binding.span;
    match checker
        .locals
        .declare(&declared_name, elem_ty.clone(), false)
    {
        crate::scope::DeclareOutcome::Fresh(info) => {
            let body = lower_block(checker, &f.body, &KaiType::Unit);
            Some(TypedStmt::For(TypedFor {
                binding_local: info.id,
                binding_name: declared_name,
                iterable,
                body,
                // Filled by the ownership pass once it runs.
                iterable_owned: false,
            }))
        }
        crate::scope::DeclareOutcome::Duplicate(_) => {
            checker.error(error::duplicate_local(&declared_name, binding_span));
            None
        }
    }
}
