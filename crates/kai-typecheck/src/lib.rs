//! Type checker: untyped AST -> TAST. The only phase allowed to convert
//! surface type names into concrete `KaiType`s, resolve variable names, and
//! enforce mutability (§9.3).

mod checker;
pub mod decl;
pub mod error;
pub mod sql;
pub mod expr;
mod scope;
pub mod stmt;
#[cfg(test)]
pub mod test_support;
pub mod ty;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;
use kai_resolver::Resolution;
use kai_tast::TypedProgram;

/// Typecheck-only entry (unit tests / tools): no name resolution performed,
/// so surface names cannot be resolved beyond primitives. The pipeline uses
/// `analyze` + `check_with`.
pub fn check(program: &Program) -> Result<TypedProgram, Vec<Diagnostic>> {
    check_with(program, &Resolution::default(), std::collections::HashMap::new())
}

/// Lowers a full program to TAST using the resolver's name tables. Returns
/// every diagnostic found; the TAST is only produced when none occurred, so
/// downstream phases can trust it fully.
pub fn check_with(
    program: &Program,
    resolution: &Resolution,
    snapshots: std::collections::HashMap<u32, crate::sql::snapshot::SqlSnapshot>,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut state = checker::Checker::new(resolution);
    state.snapshots = snapshots;
    let typed = decl::program(&mut state, program);

    if !state.failed() {
        Ok(typed)
    } else {
        Err(state.diagnostics)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod v0003_tests;
#[cfg(test)]
mod v0004_tests;
#[cfg(test)]
mod v0005_tests;
#[cfg(test)]
mod v0006_tests;
#[cfg(test)]
mod v0005_string_extra;
#[cfg(test)]
mod v0010_tests;

pub fn check_with_schema(
    program: &Program,
    resolution: &Resolution,
    snapshots: std::collections::HashMap<u32, crate::sql::snapshot::SqlSnapshot>,
    current_schema: Option<crate::sql::snapshot::SqlSnapshot>,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut state = checker::Checker::new(resolution);
    state.snapshots = snapshots;
    state.current_schema = current_schema;
    let typed = decl::program(&mut state, program);

    if !state.failed() {
        Ok(typed)
    } else {
        Err(state.diagnostics)
    }
}
