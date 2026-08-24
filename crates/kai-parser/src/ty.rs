//! Types in syntactic position. v0.0.6 adds the builtin parameterized types
//! (`Optional<T>`, `Result<T, E>`), closure types (`(T) -> R`), and the
//! canonical `T?` sugar that desugars straight to `Optional<T>` (§9.9a).

use crate::error;
use crate::parser::Parser;
use kai_ast::{Ident, Ty};
use kai_lexer::TokenKind;

/// `Type ::= BaseType { '[' ']' } [ '?' ]` — the bracket pair binds postfix,
/// so nested array shapes (`int32[][]`) parse by repetition; the `?` sugar
/// applies to whatever base it follows (`string[]?` is `Optional<string[]>`).
/// `??` lexes as a single token, so a doubled suffix can never re-enter here.
pub fn ty(parser: &mut Parser) -> Ty {
    let mut base = base_ty(parser);
    while parser.peek().kind == TokenKind::LBracket
        && matches!(parser.peek_ahead_kind(), Some(TokenKind::RBracket))
    {
        parser.bump(); // `[`
        let close = parser.expect_simple(&TokenKind::RBracket);
        let _ = close;
        base = Ty::Array(Box::new(base));
    }
    if parser.eat_simple(&TokenKind::Question) {
        // One semantic form: the sugar folds away before typecheck (§9.9a).
        base = Ty::Optional(Box::new(base));
    }
    base
}

fn base_ty(parser: &mut Parser) -> Ty {
    match parser.peek().kind.clone() {
        TokenKind::LParen => closure_ty(parser),
        TokenKind::Ident(name) => named_ty(parser, name),
        _ => {
            let found = parser.peek().clone();
            parser
                .diagnostics
                .push(error::expected("a type name", &found));
            parser.bump();
            Ty::Named(Ident {
                name: String::new(),
                span: found.span,
            })
        }
    }
}

/// A plain name, possibly with generic arguments. Generic parameters exist
/// ONLY on the two builtin parameterized types in v0.0.6 (builtin-only
/// parametric machinery): `Optional<T>` takes exactly one, `Result<T, E>`
/// exactly two. Any other name followed by `<` is a diagnostic.
fn named_ty(parser: &mut Parser, head: String) -> Ty {
    let tok = parser.bump();
    let ident = Ident {
        name: head,
        span: tok.span,
    };
    if !parser.eat_simple(&TokenKind::Lt) {
        return Ty::Named(ident);
    }
    match ident.name.as_str() {
        "Optional" => {
            let inner = ty(parser);
            if parser.eat_simple(&TokenKind::Comma) {
                parser.diagnostics.push(error::custom(
                    "Optional takes exactly one type parameter",
                    parser.span_here(),
                ));
                skip_type_args(parser);
            }
            parser.expect_simple(&TokenKind::Gt);
            Ty::Optional(Box::new(inner))
        }
        "Result" => {
            let ok = ty(parser);
            if !parser.eat_simple(&TokenKind::Comma) {
                let found = parser.peek().clone();
                parser.diagnostics.push(error::custom(
                    format!(
                        "Result takes two type parameters (ok, err); expected `,`, found {}",
                        found.describe()
                    ),
                    found.span,
                ));
                parser.expect_simple(&TokenKind::Gt);
                return Ty::Result {
                    ok: Box::new(ok),
                    err: Box::new(Ty::Named(Ident {
                        name: String::new(),
                        span: found.span,
                    })),
                };
            }
            let err = ty(parser);
            parser.expect_simple(&TokenKind::Gt);
            Ty::Result {
                ok: Box::new(ok),
                err: Box::new(err),
            }
        }
        _ => {
            let name = ident.name.clone();
            parser.diagnostics.push(error::custom(
                format!("`{name}` cannot take type parameters (only Optional and Result)"),
                ident.span,
            ));
            skip_type_args(parser);
            Ty::Named(ident)
        }
    }
}

/// Consumes up to the matching `>` of an already-reported-bad generic argument
/// list so parsing can continue without cascading on every inner token.
fn skip_type_args(parser: &mut Parser) {
    let mut depth = 1usize;
    while depth > 0 && !parser.at_eof() {
        match parser.peek().kind {
            TokenKind::Lt => depth += 1,
            TokenKind::Gt => depth -= 1,
            _ => {}
        }
        parser.bump();
    }
}

/// `'(' [ Type { ',' Type } ] ')' '->' Type` — closure type. The arrow is
/// mandatory: `(int32)` alone is not a type form in Kai.
fn closure_ty(parser: &mut Parser) -> Ty {
    parser.expect_simple(&TokenKind::LParen);
    let mut params = Vec::new();
    if parser.peek().kind != TokenKind::RParen {
        loop {
            params.push(ty(parser));
            if !parser.eat_simple(&TokenKind::Comma) {
                break;
            }
            if matches!(parser.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                break;
            }
        }
    }
    parser.expect_simple(&TokenKind::RParen);
    parser.expect_simple(&TokenKind::Arrow);
    let ret = ty(parser);
    Ty::Closure {
        params,
        ret: Box::new(ret),
    }
}
