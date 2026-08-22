//! Expressions. v0.0.1 supports integer literals only; precedence levels land
//! in v0.0.2 (each level becomes its own function here).

use crate::error;
use crate::parser::Parser;
use kai_ast::{Expr, ExprKind, IntLit};
use kai_lexer::TokenKind;

pub fn expr(parser: &mut Parser) -> Expr {
    let token = parser.peek().clone();
    match token.kind {
        TokenKind::IntLit(value) => {
            parser.bump();
            Expr {
                kind: ExprKind::IntLit(IntLit {
                    value,
                    span: token.span,
                }),
                span: token.span,
            }
        }
        _ => {
            parser
                .diagnostics
                .push(error::expected("an expression", &token));
            Expr {
                kind: ExprKind::IntLit(IntLit {
                    value: 0,
                    span: token.span,
                }),
                span: token.span,
            }
        }
    }
}
