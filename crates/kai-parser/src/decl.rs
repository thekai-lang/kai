//! Top-level declarations: functions (v0.0.1) and struct types (v0.0.3).

use crate::error;
use crate::parser::Parser;
use crate::stmt;
use crate::ty;
use kai_ast::{FieldDecl, FnDecl, Param, Program, TypeDecl};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn program(parser: &mut Parser) -> Program {
    let mut fns = Vec::new();
    let mut types = Vec::new();

    while !parser.at_eof() {
        match parser.peek().kind.clone() {
            TokenKind::Fn => fns.push(fn_decl(parser)),
            TokenKind::Type => types.push(type_decl(parser)),
            _ => {
                let found = parser.peek().clone();
                parser
                    .diagnostics
                    .push(error::expected("a top-level declaration", &found));
                parser.bump();
            }
        }
    }

    Program { fns, types }
}

fn fn_decl(parser: &mut Parser) -> FnDecl {
    let start = parser.span_here();
    parser.bump(); // `fn`

    let name = parser.expect_ident("a function name");
    let params = params(parser);
    parser.expect_simple(&TokenKind::Arrow);
    let ret = ty::ty(parser);
    let body = stmt::block(parser);
    let span = Span::merge(start, body.span);

    FnDecl {
        name,
        params,
        ret,
        body,
        span,
    }
}

/// `( [ [mut] name : Type { , [mut] name : Type } ] )`
///
/// The arrow return type is mandatory in Kai (§9.3): `-> unit` for procedures.
fn params(parser: &mut Parser) -> Vec<Param> {
    parser.expect_simple(&TokenKind::LParen);
    let mut out = Vec::new();

    if parser.peek().kind == TokenKind::RParen {
        parser.bump(); // `)`
        return out;
    }

    loop {
        let mutable = parser.eat_simple(&TokenKind::Mut);
        let name = parser.expect_ident("a parameter name");
        parser.expect_simple(&TokenKind::Colon);
        let ty = ty::ty(parser);
        out.push(Param { name, ty, mutable });

        if !parser.eat_simple(&TokenKind::Comma) {
            break;
        }
        if matches!(parser.peek().kind, TokenKind::RParen | TokenKind::Eof) {
            break; // trailing comma tolerated at the list end
        }
    }

    parser.expect_simple(&TokenKind::RParen);
    out
}

/// `type Name = { FieldDecl* }` — struct with named fields (§9.2). No `;`
/// after the closing brace; field lines are semicolon-terminated instead.
fn type_decl(parser: &mut Parser) -> TypeDecl {
    let start = parser.span_here();
    parser.bump(); // `type`

    let name = parser.expect_ident("a type name");
    parser.expect_simple(&TokenKind::Eq);
    parser.expect_simple(&TokenKind::LBrace);

    let mut fields = Vec::new();
    while !matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
        fields.push(field_decl(parser));
    }

    let end_brace = parser.expect_simple(&TokenKind::RBrace);
    let span = Span::merge(start, end_brace);

    TypeDecl { name, fields, span }
}

fn field_decl(parser: &mut Parser) -> FieldDecl {
    let name = parser.expect_ident("a field name");
    parser.expect_simple(&TokenKind::Colon);
    let ty = ty::ty(parser);
    parser.expect_simple(&TokenKind::Semi);
    FieldDecl { name, ty }
}
