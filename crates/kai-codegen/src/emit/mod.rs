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
    let fn_type = types::fn_signature(ctx, decl.ret);
    let function = ctx.module.add_function(&decl.name, fn_type, None);
    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let mut frame = crate::frame::Frame::new();
    for stmt in &decl.body.stmts {
        stmt::emit(ctx, &mut frame, stmt);
    }

    // Control flow can leave the last block unterminated (both `if` arms
    // returned); close it with a dead fallback return so the module verifies.
    stmt::fallback_return(ctx, decl.ret);
}
