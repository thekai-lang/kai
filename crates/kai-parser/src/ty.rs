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
    loop {
        if parser.peek().kind == TokenKind::LBracket
            && matches!(parser.peek_ahead_kind(), Some(TokenKind::RBracket))
        {
            parser.bump(); // `[`
            let close = parser.expect_simple(&TokenKind::RBracket);
            let _ = close;
            base = Ty::Array(Box::new(base));
            continue;
        }
        if parser.eat_simple(&TokenKind::Question) {
            // One semantic form: the sugar folds away before typecheck (§9.9a).
            base = Ty::Optional(Box::new(base));
            continue;
        }
        if parser.peek().kind == TokenKind::At {
            base = temporal_ty(parser, base);
            continue;
        }
        break;
    }
    base
}

fn temporal_ty(parser: &mut Parser, inner: Ty) -> Ty {
    let at_span = parser.expect_simple(&TokenKind::At);
    let origin = match parser.peek().kind.clone() {
        TokenKind::LocalKw => {
            parser.bump();
            kai_ast::TemporalOrigin::Local
        }
        TokenKind::WallclockKw => {
            parser.bump();
            kai_ast::TemporalOrigin::Wallclock
        }
        _ => {
            let found = parser.peek().clone();
            parser.diagnostics.push(error::expected("`local` or `wallclock`", &found));
            kai_ast::TemporalOrigin::Local
        }
    };
    parser.expect_simple(&TokenKind::LParen);
    let duration = duration_lit(parser);
    let end = parser.expect_simple(&TokenKind::RParen);
    let _span = kai_diagnostics::Span::merge(at_span, end);
    Ty::Temporal {
        inner: Box::new(inner),
        origin,
        duration,
    }
}

fn duration_lit(parser: &mut Parser) -> kai_ast::DurationLit {
    let tok = parser.peek().clone();
    match tok.kind {
        TokenKind::DurationLit { value, unit } => {
            parser.bump();
            let span = tok.span;
            let ast_unit = match unit {
                kai_lexer::token::DurationUnit::Ms => kai_ast::DurationUnit::Ms,
                kai_lexer::token::DurationUnit::S => kai_ast::DurationUnit::S,
                kai_lexer::token::DurationUnit::M => kai_ast::DurationUnit::M,
                kai_lexer::token::DurationUnit::H => kai_ast::DurationUnit::H,
                kai_lexer::token::DurationUnit::D => kai_ast::DurationUnit::D,
            };
            kai_ast::DurationLit { value, unit: ast_unit, span }
        }
        _ => {
            parser.diagnostics.push(error::expected("a duration literal (e.g. `30m`)", &tok));
            // Recover with 0m
            kai_ast::DurationLit {
                value: 0,
                unit: kai_ast::DurationUnit::M,
                span: tok.span,
            }
        }
    }
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
            Ty::Path(vec![Ident { name: String::new(), span: found.span }])
        }
    }
}

/// A plain name, possibly with generic arguments. Generic parameters exist
/// ONLY on the two builtin parameterized types in v0.0.6 (builtin-only
/// parametric machinery): `Optional<T>` takes exactly one, `Result<T, E>`
/// exactly two. Any other name followed by `<` is a diagnostic.
fn named_ty(parser: &mut Parser, head: String) -> Ty {
    let tok = parser.bump();
    let mut path = vec![Ident {
        name: head,
        span: tok.span,
    }];
    
    while parser.eat_simple(&TokenKind::Dot) {
        path.push(parser.expect_ident("a type name segment"));
    }

    if !parser.eat_simple(&TokenKind::Lt) {
        return Ty::Path(path);
    }
    match path.last().unwrap().name.as_str() {
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
                    err: Box::new(Ty::Path(vec![Ident {
                        name: String::new(),
                        span: found.span,
                    }])),
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
            let name = path.last().unwrap().name.clone();
            parser.diagnostics.push(error::custom(
                format!("`{name}` cannot take type parameters (only Optional and Result)"),
                path.last().unwrap().span,
            ));
            skip_type_args(parser);
            Ty::Path(path)
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
