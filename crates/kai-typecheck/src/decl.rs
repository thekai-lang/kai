//! Function-declaration lowering: resolve signatures, lower bodies, enforce
//! the "must actually return" rule via definite-return analysis.

use crate::checker::Checker;
use crate::error;
use crate::scope::Locals;
use crate::stmt;
use crate::ty;
use kai_ast::Program;
use kai_tast::{FunctionId, KaiType, TypedBlock, TypedFnDecl, TypedProgram};

pub(crate) fn program(checker: &mut Checker, program: &Program) -> TypedProgram {
    let fns = program
        .fns
        .iter()
        .enumerate()
        .map(|(id, decl)| {
            // Each function starts from a clean local-variable slate.
            checker.locals = Locals::new();
            fn_decl(checker, decl, FunctionId(id as u32))
        })
        .collect();

    TypedProgram { fns }
}

fn fn_decl(checker: &mut Checker, decl: &kai_ast::FnDecl, id: FunctionId) -> TypedFnDecl {
    let ret = ty::resolve(checker, &decl.ret);
    let body = stmt::lower_block(checker, &decl.body, ret);

    ensure_returns_on_all_paths(checker, decl, ret, &body);

    TypedFnDecl {
        id,
        name: decl.name.name.clone(),
        ret,
        body,
    }
}

/// A block definitely returns when its last statement is a `return`, or an
/// `if/else` whose both arms definitely return (§9.4).
fn definitely_returns(block: &TypedBlock) -> bool {
    match block.stmts.last() {
        Some(kai_tast::TypedStmt::Return(_)) => true,
        Some(kai_tast::TypedStmt::If(if_)) => {
            let then_ret = definitely_returns(&if_.then_block);
            let else_ret = if_.else_block.as_ref().is_some_and(definitely_returns);
            then_ret && else_ret
        }
        _ => false,
    }
}

fn ensure_returns_on_all_paths(
    checker: &mut Checker,
    decl: &kai_ast::FnDecl,
    ret: KaiType,
    body: &TypedBlock,
) {
    if ret == KaiType::Unit || definitely_returns(body) {
        return;
    }
    let span = decl.span;
    let name = decl.name.name.clone();
    checker.error(error::function_needs_return(&name, ret, span));
}
