//! Function emission: struct declarations, function declarations (pass 1),
//! then bodies (pass 2) so out-of-order and recursive calls resolve.

pub(crate) mod expr;
pub(crate) mod ownership;
pub(crate) mod ownership_tagged;
pub(crate) mod panic;
pub(crate) mod reversible;
pub(crate) mod stmt;
pub(crate) mod wallclock;
pub(crate) use stmt::fallback_return;

use crate::context::Ctx;
use crate::frame::Frame;
use crate::types;
use inkwell::types::BasicTypeEnum;
use kai_tast::{StructId, TypedProgram, TypedStruct};

pub(crate) fn program(ctx: &mut Ctx, program: &TypedProgram) {
    declare_structs(ctx, &program.structs);

    for decl in &program.fns {
        let param_tys: Vec<BasicTypeEnum> = decl
            .params
            .iter()
            .map(|p| types::to_llvm(ctx, &p.ty))
            .collect();
        let fn_type = types::fn_signature(ctx, &decl.ret, &param_tys);
        // Module-qualified symbol: same-named functions in different modules
        // never collide inside one LLVM module. Entry-module names stay bare
        // (`main` must be linkable/JIT-callable).
        let symbol = qualified_name(&decl.module, &decl.name);
        let function = ctx.module.add_function(&symbol, fn_type, None);
        for (idx, param) in decl.params.iter().enumerate() {
            let idx = idx as u32;
            function
                .get_nth_param(idx)
                .expect("parameter exists")
                .set_name(&param.name);
        }
        ctx.functions.push(function);
    }

    // Bodies emit after every signature exists: calls and recursion never
    // depend on definition order.
    for decl in &program.fns {
        function_body(ctx, decl);
    }
}

/// `%module.Name = type { .. }` — qualified like fn symbols so same-named
/// structs in different modules stay distinct.
fn declare_structs(ctx: &mut Ctx, structs: &[TypedStruct]) {
    for ts in structs {
        let name = qualified_name(&ts.module, &ts.name);
        let llvm_ty = ctx.context.opaque_struct_type(&name);
        let idx = ctx.structs.len() as u32;
        ctx.structs.push(llvm_ty);
        // Kai-side field types ride along for ownership helper generation.
        let field_kais: Vec<kai_tast::KaiType> =
            ts.fields.iter().map(|f| f.ty.clone()).collect();
        ctx.declare_struct_fields(idx, field_kais);
    }
    for (idx, ts) in structs.iter().enumerate() {
        let field_tys: Vec<BasicTypeEnum> = ts
            .fields
            .iter()
            .map(|f| types::to_llvm(ctx, &f.ty))
            .collect();
        ctx.structs[idx].set_body(&field_tys, false);
    }
}

/// `""` (entry module) keeps the bare name; everything else is prefixed
/// `module.name`.
fn qualified_name(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}.{name}")
    }
}

fn function_body<'ctx>(ctx: &Ctx<'ctx>, decl: &kai_tast::TypedFnDecl) {
    let function = ctx.functions[decl.id.0 as usize];
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let mut frame = Frame::new(decl.module.clone());
    frame.reversible = decl.is_reversible;
    ctx.reversible_active.set(decl.is_reversible);

    // §5.3.5: a `reversible` call opens a fresh per-activation ledger.
    if decl.is_reversible {
        ctx.builder
            .build_call(crate::runtime::reversible_enter_fn(ctx), &[], "rev.enter")
            .expect("kai_reversible_enter call");
    }

    // Params become read-write locals: alloca + store the incoming copy.
    // Callers passed values BY VALUE, so callee mutation stays invisible.
    for (idx, param) in decl.params.iter().enumerate() {
        let arg = function
            .get_nth_param(idx as u32)
            .expect("parameter exists");
        let slot = alloca_in_entry(ctx, function, types::to_llvm(ctx, &param.ty), &param.name);
        let _ = ctx.builder.build_store(slot, arg);
        frame.bind(param.local, slot);
    }

    for stmt in &decl.body.stmts {
        stmt::emit(ctx, &mut frame, stmt);
    }

    // Control flow can leave the last block unterminated (both `if` arms
    // returned); close it with a dead fallback return so the module verifies.
    stmt::fallback_return(ctx, &decl.ret, &frame);
    ctx.reversible_active.set(false);
}

/// Codegen invariant: every stack allocation is emitted at the top of the
/// function's entry block. What LLVM later does with them (e.g. promotion to
/// registers) is its own business.
pub(crate) fn alloca_in_entry<'ctx>(
    ctx: &Ctx<'ctx>,
    function: inkwell::values::FunctionValue<'ctx>,
    ty: BasicTypeEnum<'ctx>,
    name: &str,
) -> inkwell::values::PointerValue<'ctx> {
    let entry = function.get_first_basic_block().expect("entry block");
    let builder = ctx.context.create_builder();
    match entry.get_first_instruction() {
        Some(first) => builder.position_before(&first),
        None => builder.position_at_end(entry),
    }
    builder.build_alloca(ty, name).expect("alloca")
}

/// Current function inferred from the insert position; used by expression
/// emission that needs an entry-block temporary.
pub(crate) fn current_function<'ctx>(ctx: &Ctx<'ctx>) -> inkwell::values::FunctionValue<'ctx> {
    ctx.builder
        .get_insert_block()
        .expect("insert position")
        .get_parent()
        .expect("function")
}

/// Field pointer via getelementptr; `struct_id` names the LLVM type so the
/// index resolves even through nested layouts.
pub(crate) fn field_gep<'ctx>(
    ctx: &Ctx<'ctx>,
    struct_id: StructId,
    ptr: inkwell::values::PointerValue<'ctx>,
    field: u32,
    name: &str,
) -> inkwell::values::PointerValue<'ctx> {
    let llvm_ty = ctx.structs[struct_id.0 as usize];
    ctx.builder
        .build_struct_gep(llvm_ty, ptr, field, name)
        .expect("struct gep")
}
