//! Types in syntactic position. Plain names until v0.0.3; `T[]` arrays
//! arrive with v0.0.5 (optionals/results stay future grammar).

use crate::parser::Parser;
use kai_ast::{Ident, Ty};
use kai_lexer::TokenKind;

/// `Type ::= ... | Type '[' ']'` — the bracket pair binds postfix, so
/// nested array shapes (`int32[][]`) parse by repetition.
pub fn ty(parser: &mut Parser) -> Ty {
    let ident: Ident = parser.expect_ident("a type name");
    let mut base = Ty::Named(ident);
    while parser.peek().kind == TokenKind::LBracket
        && matches!(parser.peek_ahead_kind(), Some(TokenKind::RBracket))
    {
        parser.bump(); // `[`
        let close = parser.expect_simple(&TokenKind::RBracket);
        let _ = close;
        base = Ty::Array(Box::new(base));
    }
    base
}
