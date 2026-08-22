//! Function-declaration lowering: resolve signatures, lower bodies, enforce
//! the "must actually return" rule.

use crate::error;
use crate::stmt;
use crate::ty;
use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use kai_tast::{FunctionId, KaiType, TypedBlock, TypedFnDecl, TypedProgram};

pub fn program(program: &Program, diagnostics: &mut Vec<Diagnostic>) -> TypedProgram {
    let fns = program
        .fns
        .iter()
        .enumerate()
        .map(|(id, decl)| fn_decl(decl, FunctionId(id as u32), diagnostics))
        .collect();

    TypedProgram { fns }
}

fn fn_decl(
    decl: &kai_ast::FnDecl,
    id: FunctionId,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypedFnDecl {
    let ret = ty::resolve(&decl.ret, decl.ret.span(), diagnostics);
    let body: TypedBlock = stmt::block(&decl.body, ret, diagnostics);

    ensure_has_return(decl, ret, &body, diagnostics);

    TypedFnDecl {
        id,
        name: decl.name.name.clone(),
        ret,
        body,
    }
}

/// v0.0.1 simplification: every non-unit function must contain a `return`
/// statement somewhere in its body, otherwise the emitted LLVM function would
/// fall through without a value. Proper flow analysis arrives with `if/else`.
fn ensure_has_return(
    decl: &kai_ast::FnDecl,
    ret: KaiType,
    body: &TypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let has_return = body
        .stmts
        .iter()
        .any(|stmt| matches!(stmt, kai_tast::TypedStmt::Return(_)));
    if !has_return {
        diagnostics.push(error::function_needs_return(
            &decl.name.name,
            ret,
            decl.span,
        ));
    }
}
