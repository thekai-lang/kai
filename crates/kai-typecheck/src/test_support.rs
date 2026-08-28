//! Shared test helpers: lex + parse a source string, panicking on failure.
//! Not compiled into release builds of dependents.

use kai_ast::Program;

pub const MAIN_OK: &str = "fn main() -> int32 {\n    return 0;\n}\n";

#[track_caller]
pub fn parse_ok(src: &str) -> Program {
    let lexed = kai_lexer::lex(src);
    assert!(
        lexed.diagnostics.is_empty(),
        "lex failed: {:?}",
        lexed.diagnostics
    );
    kai_parser::parse(&lexed.tokens).expect("parse failed")
}

pub fn check_source_with_all_snapshots(
    source: &str,
    sql_snapshots: std::collections::HashMap<u32, crate::sql::snapshot::SqlSnapshot>,
    api_snapshots: std::collections::HashMap<(String, u32), crate::api::snapshot::ApiSnapshot>,
) -> Result<kai_tast::TypedProgram, Vec<kai_diagnostics::Diagnostic>> {
    use kai_resolver::Resolution;
    
    let ast = parse_ok(source);
    let resolution = match kai_resolver::analyze(&ast) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    
    crate::check_with(&ast, &resolution, sql_snapshots, api_snapshots)
}
