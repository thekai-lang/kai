//! Function emission: signatures, entry blocks, bodies.

pub(crate) mod expr;
pub(crate) mod stmt;

use crate::context::Ctx;
use crate::types;

pub(crate) fn program(ctx: &Ctx, program: &kai_tast::TypedProgram) {
    for decl in &program.fns {
        function(ctx, decl);
    }
}

fn function(ctx: &Ctx, decl: &kai_tast::TypedFnDecl) {
    let ret_ty = types::to_llvm(ctx, decl.ret);
    let fn_type = ret_ty.fn_type(&[], false);
    let function = ctx.module.add_function(&decl.name, fn_type, None);
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    for stmt in &decl.body.stmts {
        // Infallible for valid TAST: every statement terminates the block.
        stmt::emit(ctx, stmt);
    }
}
