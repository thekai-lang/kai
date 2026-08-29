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
            // Each function starts from a clean local-variable slate and a
            // fresh module context (drives unqualified lookups AND the file
            // stamped onto diagnostics).
            checker.locals = Locals::new();
            checker.current_module = owner_module(checker, id);
            checker.cur_file = owner_file(checker, id);
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
        .enumerate()
        .map(|(idx, decl)| {
            // Field types resolve through the OWNING module's table.
            checker.current_module = type_owner(checker, idx);
            checker.cur_file = type_file(checker, idx);
            kai_tast::TypedStruct {
                name: decl.name.name.clone(),
                module: module_path(checker, checker.current_module),
                fields: decl
                    .fields
                    .iter()
                    .map(|field| kai_tast::TypedStructField {
                        name: field.name.name.clone(),
                        ty: ty::resolve(checker, &field.ty),
                    })
                    .collect(),
            }
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
                    ty: f.ty.clone(),
                })
                .collect(),
        })
        .collect();

    structs
}

/// Same pre-pass for function signatures: param/return types resolve against
/// the now-complete struct table, through each function's OWN module.
fn build_fn_signatures(checker: &mut Checker, program: &Program) {
    for (idx, decl) in program.fns.iter().enumerate() {
        checker.current_module = owner_module(checker, idx);
        checker.cur_file = owner_file(checker, idx);
        let param_tys = decl
            .params
            .iter()
            .map(|p| ty::resolve(checker, &p.ty))
            .collect();
        let ret = ty::resolve(checker, &decl.ret);
        checker.fns.push(crate::checker::FnInfo {
            name: decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join("."),
            param_tys,
            ret,
        });
    }
}

/// Dotted module path (`""` = entry) — travels with TAST decls so codegen
/// can qualify symbol names without consulting resolution.
fn module_path(checker: &Checker, idx: usize) -> String {
    checker.resolution.module_names[idx].clone()
}

// Legacy callers (`check` without a resolution) pass an empty Resolution:
// every declaration then belongs to the anonymous entry module.
fn owner_module(checker: &Checker, idx: usize) -> usize {
    checker.resolution.fn_module.get(idx).copied().unwrap_or(0)
}

fn owner_file(checker: &Checker, idx: usize) -> String {
    checker
        .resolution
        .fn_file
        .get(idx)
        .cloned()
        .unwrap_or_default()
}

fn type_owner(checker: &Checker, idx: usize) -> usize {
    checker
        .resolution
        .type_module
        .get(idx)
        .copied()
        .unwrap_or(0)
}

fn type_file(checker: &Checker, idx: usize) -> String {
    checker
        .resolution
        .type_file
        .get(idx)
        .cloned()
        .unwrap_or_default()
}

fn fn_decl(checker: &mut Checker, decl: &kai_ast::FnDecl, id: FunctionId) -> TypedFnDecl {
    let ret = ty::resolve(checker, &decl.ret);
    let params = bind_params(checker, decl);
    let body = stmt::lower_block(checker, &decl.body, &ret);

    ensure_returns_on_all_paths(checker, decl, &ret, &body);

    let declared_effects = decl.effects.as_ref().map(|set| {
        let effects = set
            .0
            .iter()
            .map(|e| match e {
                kai_ast::EffectName::EscapesLocalContext => kai_tast::Effect::EscapesLocalContext,
            })
            .collect();
        kai_tast::EffectSet(effects)
    });
    TypedFnDecl {
        id,
        name: decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join("."),
        module: module_path(checker, checker.current_module),
        params,
        ret,
        declared_effects,
        inferred_effects: kai_tast::EffectSet::default(),
        is_reversible: decl.is_reversible,
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
                .declare(&param.name.name, param_ty.clone(), param.mutable)
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
pub(crate) fn definitely_returns(block: &TypedBlock) -> bool {
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
    ret: &KaiType,
    body: &TypedBlock,
) {
    if *ret == KaiType::Unit || definitely_returns(body) {
        return;
    }
    let span = decl.span;
    let name = decl.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
    checker.error(error::function_needs_return(&name, ret.clone(), span));
}
