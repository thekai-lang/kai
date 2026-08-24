//! Module-level finalization: verification and IR printing.

use crate::context::Ctx;

pub(crate) fn verify(ctx: &Ctx) -> Result<(), String> {
    ctx.module.verify().map_err(|e| {
        let _ = std::fs::write("/tmp/opencode/kai6/fail.ll", print(ctx));
        e.to_string()
    })
}

pub(crate) fn print(ctx: &Ctx) -> String {
    ctx.module.print_to_string().to_string()
}
