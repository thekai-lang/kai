//! §5.2/v0.21 host sinks: `.kai/observe.log` (Signal telemetry) and
//! `.kai/debt.log` (pre-ledger `require` violations, §10.3). The JIT'd
//! program passes baked strings; this module owns timestamp acquisition,
//! JSONL formatting via the kai-effects Trust<C> record shapes (§5.2.1–
//! §5.2.2), and append-with-flush so a panic after a debt record cannot
//! lose it (§10.3 sequencing: record BEFORE exit).

use std::ffi::{c_char, CStr};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// UTC microseconds since epoch — same instant shape as §5.1.7 headers.
fn micros_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

unsafe fn cstr<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller guarantees a NUL-terminated baked global.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

fn append_line(sink_path: &str, line: &str) {
    let path = Path::new(sink_path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
}

/// Records one `observe` Signal evaluation (§5.2.2): timestamp acquired
/// here, JSONL shaped by `kai_effects::trust::observe_jsonl`, appended to
/// the sink file under the project root.
///
/// # Safety
/// All three pointers must be NUL-terminated baked globals or null (null =
/// silently skip; only reachable from root-less string-API builds, which
/// never bake these calls).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_observe_record(
    sink_path: *const c_char,
    location: *const c_char,
    condition: *const c_char,
    outcome: i32,
) {
    let (Some(sink), Some(loc), Some(cond)) = (
        unsafe { cstr(sink_path) },
        unsafe { cstr(location) },
        unsafe { cstr(condition) },
    ) else {
        return;
    };
    let line =
        kai_effects::trust::observe_jsonl(micros_now(), loc, cond, outcome != 0);
    append_line(sink, &line);
}

/// Records one pre-ledger `require` violation (§10.3): synchronous,
/// flushed, called BEFORE `kai_panic` exits the process.
///
/// # Safety
/// Same contract as [`kai_observe_record`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kai_debt_record(
    sink_path: *const c_char,
    location: *const c_char,
    condition: *const c_char,
) {
    let (Some(sink), Some(loc), Some(cond)) = (
        unsafe { cstr(sink_path) },
        unsafe { cstr(location) },
        unsafe { cstr(condition) },
    ) else {
        return;
    };
    let line = kai_effects::trust::debt_correctness_jsonl(micros_now(), loc, cond);
    append_line(sink, &line);
}

// -- LLVM-side declarations ----------------------------------------------------

use inkwell::values::FunctionValue;

use super::{get_or_declare, Ctx};

/// `kai_observe_record(ptr sink, ptr loc, ptr cond, i32 outcome)` — void.
pub(crate) fn observe_record_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(
        &[ptr.into(), ptr.into(), ptr.into(), ctx.context.i32_type().into()],
        false,
    );
    get_or_declare(ctx, "kai_observe_record", llvm)
}

/// `kai_debt_record(ptr sink, ptr loc, ptr cond)` — void.
pub(crate) fn debt_record_fn<'ctx>(ctx: &Ctx<'ctx>) -> FunctionValue<'ctx> {
    let ptr = ctx.context.ptr_type(Default::default());
    let llvm = ctx.context.void_type().fn_type(&[ptr.into(), ptr.into(), ptr.into()], false);
    get_or_declare(ctx, "kai_debt_record", llvm)
}
