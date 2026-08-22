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
