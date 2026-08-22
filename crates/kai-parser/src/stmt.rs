//! Statements: blocks and the v0.0.1 statement forms.

use crate::error;
use crate::expr;
use crate::parser::Parser;
use kai_ast::{Block, Stmt, StmtKind};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn block(parser: &mut Parser) -> Block {
    let start = parser.span_here();
    parser.expect_simple(&TokenKind::LBrace);

    let mut stmts = Vec::new();
    loop {
        match parser.peek().kind.clone() {
            TokenKind::RBrace => break,
            TokenKind::Eof => break,
            _ => stmts.push(stmt(parser)),
        }
    }

    let end = parser.expect_simple(&TokenKind::RBrace);
    Block {
        stmts,
        span: Span::merge(start, end),
    }
}

fn stmt(parser: &mut Parser) -> Stmt {
    match parser.peek().kind.clone() {
        TokenKind::Return => ret(parser),
        _ => {
            let found = parser.peek().clone();
            parser
                .diagnostics
                .push(error::expected("a statement", &found));
            parser.bump();
            Stmt {
                kind: StmtKind::Return(None),
                span: found.span,
            }
        }
    }
}

fn ret(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `return`

    let value = if parser.peek().kind == TokenKind::Semi {
        None
    } else {
        Some(expr::expr(parser))
    };

    let end = parser.expect_simple(&TokenKind::Semi);
    Stmt {
        kind: StmtKind::Return(value),
        span: Span::merge(start, end),
    }
}
