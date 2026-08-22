//! Top-level declarations: function definitions (the only form in v0.0.1).

use crate::error;
use crate::parser::Parser;
use crate::stmt;
use crate::ty;
use kai_ast::{FnDecl, Program};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn program(parser: &mut Parser) -> Program {
    let mut fns = Vec::new();

    while !parser.at_eof() {
        if parser.peek().kind == TokenKind::Fn {
            fns.push(fn_decl(parser));
        } else {
            let found = parser.peek().clone();
            parser
                .diagnostics
                .push(error::expected("a top-level declaration", &found));
            parser.bump();
        }
    }

    Program { fns }
}

fn fn_decl(parser: &mut Parser) -> FnDecl {
    let start = parser.span_here();
    parser.bump(); // `fn`

    let name = parser.expect_ident("a function name");
    params(parser);
    parser.expect_simple(&TokenKind::Arrow);
    let ret = ty::ty(parser);
    let body = stmt::block(parser);
    let span = Span::merge(start, body.span);

    FnDecl {
        name,
        params: Vec::new(),
        ret,
        body,
        span,
    }
}

/// v0.0.1 functions take no parameters; the parenthesized list must be empty.
fn params(parser: &mut Parser) {
    parser.expect_simple(&TokenKind::LParen);
    if parser.peek().kind != TokenKind::RParen {
        let span = parser.span_here();
        parser.diagnostics.push(error::custom(
            "parameters are not supported until v0.0.3",
            span,
        ));
        while !parser.at_eof() && !parser.eat_simple(&TokenKind::RParen) {
            parser.bump();
        }
    } else {
        parser.bump(); // `)`
    }
}
