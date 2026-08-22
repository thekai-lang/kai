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
    let structs = build_struct_layouts(checker, program);
    build_fn_signatures(checker, program);

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

    TypedProgram { structs, fns }
}

/// Resolves every struct's field types once, in declaration order, so
/// `StructId` doubles as an index into the layout table. Cycles were
/// rejected by the resolver; unknown field types error here. The resolved
/// layouts travel with the TAST — codegen rebuilds them as LLVM types.
fn build_struct_layouts(checker: &mut Checker, program: &Program) -> Vec<kai_tast::TypedStruct> {
    let structs: Vec<kai_tast::TypedStruct> = program
        .types
        .iter()
        .map(|decl| kai_tast::TypedStruct {
            name: decl.name.name.clone(),
            fields: decl
                .fields
                .iter()
                .map(|field| kai_tast::TypedStructField {
                    name: field.name.name.clone(),
                    ty: ty::resolve(checker, &field.ty),
                })
                .collect(),
        })
        .collect();

    // Mirror into the checker's own lookup table for expression typing.
    checker.structs = structs
        .iter()
        .map(|ts| crate::checker::StructLayout {
            name: ts.name.clone(),
            fields: ts
                .fields
                .iter()
                .map(|f| crate::checker::FieldSlot {
                    name: f.name.clone(),
                    ty: f.ty,
                })
                .collect(),
        })
        .collect();

    structs
}

/// Same pre-pass for function signatures: param/return types resolve against
/// the now-complete struct table.
fn build_fn_signatures(checker: &mut Checker, program: &Program) {
    for decl in &program.fns {
        let param_tys = decl
            .params
            .iter()
            .map(|p| ty::resolve(checker, &p.ty))
            .collect();
        let ret = ty::resolve(checker, &decl.ret);
        checker.fns.push(crate::checker::FnInfo {
            name: decl.name.name.clone(),
            param_tys,
            ret,
        });
    }
}

fn fn_decl(checker: &mut Checker, decl: &kai_ast::FnDecl, id: FunctionId) -> TypedFnDecl {
    let ret = ty::resolve(checker, &decl.ret);
    let params = bind_params(checker, decl);
    let body = stmt::lower_block(checker, &decl.body, ret);

    ensure_returns_on_all_paths(checker, decl, ret, &body);

    TypedFnDecl {
        id,
        name: decl.name.name.clone(),
        params,
        ret,
        body,
    }
}

/// Declares parameters as function-root locals. `mut` on a stack-type param
/// is a purely local permission (§9.3); it never changes the ABI.
fn bind_params(checker: &mut Checker, decl: &kai_ast::FnDecl) -> Vec<kai_tast::TypedParam> {
    decl.params
        .iter()
        .filter_map(|param| {
            let param_ty = ty::resolve(checker, &param.ty);
            match checker
                .locals
                .declare(&param.name.name, param_ty, param.mutable)
            {
                crate::scope::DeclareOutcome::Fresh(info) => Some(kai_tast::TypedParam {
                    local: info.id,
                    name: param.name.name.clone(),
                    ty: param_ty,
                }),
                crate::scope::DeclareOutcome::Duplicate(_) => {
                    checker.error(error::duplicate_local(&param.name.name, param.name.span));
                    None
                }
            }
        })
        .collect()
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
