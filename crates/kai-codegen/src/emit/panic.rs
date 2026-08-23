//! Runtime panic emission (§10.1): every runtime check branches to a
//! dedicated block that reports `message at file:line:col` and exits 101.
//! Control never returns — the block ends in `unreachable`.

use inkwell::basic_block::BasicBlock;
use kai_diagnostics::Span;

use crate::context::Ctx;

/// Builds a fresh panic block for the function under construction and
/// leaves the builder positioned in it. Callers keep emitting into their
/// own block first, then branch to the returned one (typically as the
/// false edge of a guard comparison). `module_key` selects the source
/// whose span is being reported (`""` = entry module).
#[allow(dead_code)] // referenced once the §10 checks emit call sites
pub(crate) fn panic_block<'ctx>(
    ctx: &Ctx<'ctx>,
    module_key: &str,
    span: Span,
    message: &str,
) -> BasicBlock<'ctx> {
    let current = ctx.builder.get_insert_block().expect("active block");
    let function = current.get_parent().expect("block in a function");
    let bb = ctx.context.append_basic_block(function, "panic");

    ctx.builder.position_at_end(bb);
    let msg = ctx
        .builder
        .build_global_string_ptr(message, "kai.panic.msg")
        .expect("message global");
    let file = file_global(ctx, module_key);
    let (line, col) = ctx
        .sources
        .get(module_key)
        .map_or((0, 0), |src| src.line_col(span.start));

    let i64_ty = ctx.context.i64_type();
    let args = [
        msg.as_pointer_value().into(),
        i64_ty.const_int(message.len() as u64, false).into(),
        file.into(),
        i64_ty.const_int(line as u64, false).into(),
        i64_ty.const_int(col as u64, false).into(),
    ];
    ctx.builder
        .build_call(crate::runtime::panic_fn(ctx), &args, "panic.call")
        .expect("kai_panic call");
    ctx.builder
        .build_unreachable()
        .expect("panic never returns");

    bb
}

/// The owning module's display path as a baked global, cached per key so a
/// function with many checks shares one string.
#[allow(dead_code)]
fn file_global<'ctx>(
    ctx: &Ctx<'ctx>,
    module_key: &str,
) -> inkwell::values::PointerValue<'ctx> {
    if let Some(existing) = ctx.file_globals.borrow().get(module_key) {
        return *existing;
    }
    let file = ctx
        .sources
        .get(module_key)
        .map_or("<unknown>", |src| src.file.as_str());
    let global = ctx
        .builder
        .build_global_string_ptr(file, "kai.src.file")
        .expect("file name global");
    let ptr = global.as_pointer_value();
    ctx.file_globals
        .borrow_mut()
        .insert(module_key.to_string(), ptr);
    ptr
}
