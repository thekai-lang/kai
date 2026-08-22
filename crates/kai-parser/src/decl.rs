//! Top-level declarations: imports, functions, struct types.

use crate::error;
use crate::parser::Parser;
use crate::stmt;
use crate::ty;
use kai_ast::{FieldDecl, FnDecl, Param, Program, TypeDecl, UseDecl};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn program(parser: &mut Parser) -> Program {
    // EBNF: `Program ::= { UseDecl } { TopLevelDecl }` — every import must
    // precede the first declaration; one after it is a diagnostic.
    let mut use_decls = Vec::new();
    while matches!(parser.peek().kind, TokenKind::Use) {
        use_decls.push(use_decl(parser));
    }

    let mut fns = Vec::new();
    let mut types = Vec::new();

    while !parser.at_eof() {
        match parser.peek().kind.clone() {
            TokenKind::Use => {
                let found = parser.peek().clone();
                parser.diagnostics.push(error::custom(
                    "an import must appear before all declarations",
                    found.span,
                ));
                use_decls.push(use_decl(parser));
            }
            // `public fn` / `public type`: dispatch on the token after the
            // qualifier; fn_decl/type_decl consume the prefix themselves.
            TokenKind::Public => match parser.peek_ahead_kind() {
                Some(TokenKind::Type) => types.push(type_decl(parser)),
                _ => fns.push(fn_decl(parser)),
            },
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

    Program {
        use_decls,
        fns,
        types,
    }
}

/// `use a.b.c;` — path segments are plain identifiers joined by dots;
/// anything else (`..`, `/`) already fails at lex or expect_ident time.
fn use_decl(parser: &mut Parser) -> UseDecl {
    let start = parser.span_here();
    parser.bump(); // `use`

    let mut path = vec![parser.expect_ident("a module path segment")];
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a module path segment"));
    }
    let end = parser.expect_simple(&TokenKind::Semi);
    let span = Span::merge(start, end);
    UseDecl { path, span }
}

fn fn_decl(parser: &mut Parser) -> FnDecl {
    let start = parser.span_here();
    let is_public = parser.eat_simple(&TokenKind::Public);
    parser.bump(); // `fn`

    let name = parser.expect_ident("a function name");
    let params = params(parser);
    parser.expect_simple(&TokenKind::Arrow);
    let ret = ty::ty(parser);
    let body = stmt::block(parser);
    let span = Span::merge(start, body.span);

    FnDecl {
        is_public,
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
    let is_public = parser.eat_simple(&TokenKind::Public);
    parser.bump(); // `type`

    let name = parser.expect_ident("a type name");
    parser.expect_simple(&TokenKind::Eq);
    parser.expect_simple(&TokenKind::LBrace);

    let mut fields = Vec::new();
    while !matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
        let before = parser.pos();
        fields.push(field_decl(parser));
        // A malformed field consumes nothing (expect_* never advances on
        // mismatch); skipping one token guarantees the loop terminates.
        if parser.pos() == before {
            parser.bump();
        }
    }

    let end_brace = parser.expect_simple(&TokenKind::RBrace);
    let span = Span::merge(start, end_brace);

    TypeDecl {
        is_public,
        name,
        fields,
        span,
    }
}

fn field_decl(parser: &mut Parser) -> FieldDecl {
    let name = parser.expect_ident("a field name");
    parser.expect_simple(&TokenKind::Colon);
    let ty = ty::ty(parser);
    parser.expect_simple(&TokenKind::Semi);
    FieldDecl { name, ty }
}
