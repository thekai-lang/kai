//! Expressions: precedence-climbing chain, highest binding last.
//! Chain (loosest → tightest): || → && → ?? → ==/!= → relational → +- → */% → unary → primary.
//! Assignment is NOT part of the expression grammar (statement-only).

use crate::error;
use crate::parser::Parser;
use crate::{decl, stmt, ty as ty_parser};
use kai_ast::{
    ArrayLitExpr, BinaryExpr, BinaryOp, CallExpr, ClosureLitExpr, CatchExpr, CoalesceExpr, ErrLitExpr, Expr,
    ExprKind, FieldAccessExpr, FieldInit, FloatLit, IndexExpr, IntLit, OkLitExpr, SomeLitExpr, StrLitExpr,
    StructLitExpr, UnaryExpr, UnaryOp,
};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

/// Entry point for every expression production. All recursive nesting
/// (parenthesized groups today; call arguments and indexing later) funnels
/// through here, so the recursion budget bounds AST depth globally.
pub fn expr(parser: &mut Parser) -> Expr {
    parser.guarded_expr(|parser| logic_or(parser))
}

macro_rules! left_assoc_level {
    ($name:ident, $next:ident, $($token:pat => $op:expr),+ $(,)?) => {
        fn $name(parser: &mut Parser) -> Expr {
            let mut lhs = $next(parser);
            loop {
                let op = match parser.peek().kind.clone() {
                    $($token => $op,)+
                    _ => break,
                };
                let op_span = parser.span_here();
                parser.bump();
                let rhs = $next(parser);
                let span = Span::merge(lhs.span, rhs.span);
                lhs = Expr {
                    span,
                    kind: ExprKind::Binary(BinaryExpr {
                        op,
                        op_span,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    }),
                };
            }
            lhs
        }
    };
}

left_assoc_level!(logic_or, logic_and,
    TokenKind::PipePipe => BinaryOp::Or);
left_assoc_level!(logic_and, coalesce,
    TokenKind::AmpAmp => BinaryOp::And);
left_assoc_level!(equality, relational,
    TokenKind::EqEq => BinaryOp::Eq,
    TokenKind::NotEq => BinaryOp::Ne);
left_assoc_level!(relational, additive,
    TokenKind::Lt => BinaryOp::Lt,
    TokenKind::Gt => BinaryOp::Gt,
    TokenKind::Le => BinaryOp::Le,
    TokenKind::Ge => BinaryOp::Ge);
left_assoc_level!(additive, multiplicative,
    TokenKind::Plus => BinaryOp::Add,
    TokenKind::Minus => BinaryOp::Sub);
left_assoc_level!(multiplicative, unary,
    TokenKind::Star => BinaryOp::Mul,
    TokenKind::Slash => BinaryOp::Div,
    TokenKind::Percent => BinaryOp::Mod);

/// `??` is right-associative (§9.9a): `a ?? b ?? c` reads `a ?? (b ?? c)`.
/// The chain parses iteratively (collect, then fold from the right) so a
/// long coalesce chain never grows parser recursion.
fn coalesce(parser: &mut Parser) -> Expr {
    let mut parts = vec![equality(parser)];
    while parser.eat_simple(&TokenKind::QuestionQuestion) {
        parts.push(equality(parser));
    }
    let mut rhs = parts.pop().expect("at least one operand");
    while let Some(lhs) = parts.pop() {
        let span = Span::merge(lhs.span, rhs.span);
        rhs = Expr {
            span,
            kind: ExprKind::Coalesce(CoalesceExpr {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }),
        };
    }
    rhs
}

/// Prefix operators are collected iteratively (no parser recursion), but each
/// one still consumes budget: the resulting AST nests once per operator, and
/// downstream phases recurse over that depth.
fn unary(parser: &mut Parser) -> Expr {
    let mut ops: Vec<(UnaryOp, Span)> = Vec::new();
    loop {
        let op = match parser.peek().kind.clone() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            _ => None,
        };
        let Some(op) = op else { break };

        if !parser.enter_expr() {
            // Release the levels this chain already charged, then skip the
            // rest of the enclosing group without parsing it.
            for _ in &ops {
                parser.exit_expr();
            }
            let span = parser.skip_to_group_end();
            parser.depth_reported = false;
            return Expr {
                kind: ExprKind::Invalid,
                span,
            };
        }

        let op_span = parser.span_here();
        parser.bump();
        ops.push((op, op_span));
    }

    let mut operand = postfix(parser);
    for (op, op_span) in ops.into_iter().rev() {
        parser.exit_expr();
        let span = Span::merge(op_span, operand.span);
        operand = Expr {
            span,
            kind: ExprKind::Unary(UnaryExpr {
                op,
                op_span,
                operand: Box::new(operand),
            }),
        };
    }
    operand
}

/// Postfix operations, parsed iteratively so chains like `line.start.x` or
/// `f(1)(2)` never grow parser recursion (each element still nests the AST
/// once, and argument lists funnel through `expr()` for budget accounting).
fn postfix(parser: &mut Parser) -> Expr {
    let mut e = primary(parser);
    loop {
        match parser.peek().kind.clone() {
            TokenKind::Dot => {
                parser.bump(); // `.`
                let field = parser.expect_ident("a field name");
                let span = Span::merge(e.span, field.span);
                e = Expr {
                    span,
                    kind: ExprKind::FieldAccess(FieldAccessExpr {
                        base: Box::new(e),
                        field,
                    }),
                };
            }
            TokenKind::LBracket => {
                // `base[index]` — element read (v0.0.5). The index funnels
                // through expr() for budget accounting like any nesting.
                parser.bump(); // `[`
                let index = expr(parser);
                let rbracket = parser.expect_simple(&TokenKind::RBracket);
                let span = Span::merge(e.span, rbracket);
                e = Expr {
                    span,
                    kind: ExprKind::Index(IndexExpr {
                        base: Box::new(e),
                        index: Box::new(index),
                        rbracket,
                    }),
                };
            }
            TokenKind::LParen => {
                parser.bump(); // `(`
                let mut args = Vec::new();
                if parser.peek().kind != TokenKind::RParen {
                    loop {
                        args.push(expr(parser));
                        if !parser.eat_simple(&TokenKind::Comma) {
                            break;
                        }
                        // Tolerate one trailing comma before `)`.
                        if matches!(parser.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                            break;
                        }
                    }
                }
                let end = parser.expect_simple(&TokenKind::RParen);
                let span = Span::merge(e.span, end);
                e = Expr {
                    span,
                    kind: ExprKind::Call(CallExpr {
                        callee: Box::new(e),
                        args,
                    }),
                };
            }
            TokenKind::Catch => {
                // `base catch |err| { stmts.. tail }` (v0.0.6, §3.4). Postfix
                // level: binds tighter than `??` and the binary operators.
                parser.bump(); // `catch`
                parser.expect_simple(&TokenKind::Pipe);
                let err_binding = parser.expect_ident("an error variable name");
                parser.expect_simple(&TokenKind::Pipe);
                let (catch_stmts, tail, block_span) = stmt::catch_block(parser);
                let span = Span::merge(e.span, block_span);
                e = Expr {
                    span,
                    kind: ExprKind::Catch(CatchExpr {
                        base: Box::new(e),
                        err_binding,
                        stmts: catch_stmts,
                        tail: Box::new(tail),
                    }),
                }
            }
            _ => break,
        }
    }
    e
}

fn primary(parser: &mut Parser) -> Expr {
    let token = parser.peek().clone();

    // Error recovery always consumes one token so callers cannot loop forever.
    macro_rules! leaf {
        ($kind:expr) => {{
            parser.bump();
            Expr {
                kind: $kind,
                span: token.span,
            }
        }};
    }

    if let TokenKind::StrLit(text) = token.kind.clone() {
        parser.bump();
        return Expr {
            kind: ExprKind::StrLit(StrLitExpr { value: text }),
            span: token.span,
        };
    }

    match token.kind.clone() {
        TokenKind::IntLit(value) => leaf!(ExprKind::IntLit(IntLit {
            value,
            span: token.span
        })),
        TokenKind::FloatLit(value) => {
            leaf!(ExprKind::FloatLit(FloatLit {
                value,
                span: token.span
            }))
        }
        TokenKind::True => leaf!(ExprKind::BoolLit {
            value: true,
            span: token.span
        }),
        TokenKind::False => leaf!(ExprKind::BoolLit {
            value: false,
            span: token.span
        }),
        TokenKind::NoneKw => leaf!(ExprKind::NoneLit),
        TokenKind::SomeKw => {
            // `Some(expr)` — Optional construction (v0.0.6, §9.9a).
            parser.bump(); // `Some`
            parser.expect_simple(&TokenKind::LParen);
            let value = expr(parser);
            let end = parser.expect_simple(&TokenKind::RParen);
            Expr {
                span: Span::merge(token.span, end),
                kind: ExprKind::SomeLit(SomeLitExpr {
                    value: Box::new(value),
                }),
            }
        }
        TokenKind::OkKw => {
            // `Ok(expr)` — Result Ok construction (v0.14, §3.4).
            parser.bump(); // `Ok`
            parser.expect_simple(&TokenKind::LParen);
            let value = expr(parser);
            let end = parser.expect_simple(&TokenKind::RParen);
            Expr {
                span: Span::merge(token.span, end),
                kind: ExprKind::OkLit(OkLitExpr {
                    value: Box::new(value),
                }),
            }
        }
        TokenKind::ErrKw => {
            // `Err(expr)` — Result Err construction (v0.14, §3.4).
            parser.bump(); // `Err`
            parser.expect_simple(&TokenKind::LParen);
            let value = expr(parser);
            let end = parser.expect_simple(&TokenKind::RParen);
            Expr {
                span: Span::merge(token.span, end),
                kind: ExprKind::ErrLit(ErrLitExpr {
                    value: Box::new(value),
                }),
            }
        }
        TokenKind::Fn => {
            // Closure literal (v0.0.6, §3.5): `fn(params) -> T { body }`.
            // Top-level `fn` declarations are handled by decl.rs dispatch;
            // in expression position this token can only start a closure.
            parser.bump(); // `fn`
            let params = decl::params(parser);
            parser.expect_simple(&TokenKind::Arrow);
            let ret = ty_parser::ty(parser);
            let body = stmt::block(parser);
            Expr {
                span: Span::merge(token.span, body.span),
                kind: ExprKind::ClosureLit(ClosureLitExpr { params, ret, body }),
            }
        }
        TokenKind::Pipe => {
            // Lone `|` lexes fine (catch delimiter); reaching PRIMARY means
            // operator position — never valid. One targeted message instead
            // of a generic expected-expression cascade.
            parser.diagnostics.push(error::custom(
                "`|` is not an operator (did you mean `||`, or is a `catch` block missing its `|`?)",
                token.span,
            ));
            parser.bump();
            Expr {
                kind: ExprKind::Invalid,
                span: token.span,
            }
        }
        TokenKind::Ident(name) => {
            // Consume the identifier, then its dotted continuation (the
            // QualifiedName shape — `p.x`, `math.Point`, bare `Point`).
            // Only AFTER the whole path is consumed do we decide whether a
            // following `{` opens a struct literal (§9.2) or belongs to the
            // statement grammar (NO_STRUCT_LITERAL, §9.3, in if conditions).
            // The path itself folds into plain Ident/FieldAccess shapes:
            // whether the head names a module is a resolver question.
            parser.bump();
            let mut path = vec![kai_ast::Ident {
                name,
                span: token.span,
            }];
            while parser.peek().kind == TokenKind::Dot
                && matches!(parser.peek_ahead_kind(), Some(TokenKind::Ident(_)))
            {
                parser.bump(); // `.`
                path.push(parser.expect_ident("a name"));
            }

            if parser.peek().kind == TokenKind::LBrace && !parser.struct_lits_banned() {
                return struct_lit(parser, path);
            }

            let mut e = Expr {
                kind: ExprKind::Ident(path[0].clone()),
                span: token.span,
            };
            for segment in path.into_iter().skip(1) {
                let span = Span::merge(e.span, segment.span);
                e = Expr {
                    span,
                    kind: ExprKind::FieldAccess(FieldAccessExpr {
                        base: Box::new(e),
                        field: segment,
                    }),
                };
            }
            e
        }
        TokenKind::LBracket => {
            // `[e0, e1, ..]` — array literal (v0.0.5). Empty `[]` is parsed
            // fine; whether an element type exists is a typecheck question.
            parser.bump(); // `[`
            let mut elements = Vec::new();
            if parser.peek().kind != TokenKind::RBracket {
                loop {
                    elements.push(expr(parser));
                    if !parser.eat_simple(&TokenKind::Comma) {
                        break;
                    }
                    if matches!(parser.peek().kind, TokenKind::RBracket | TokenKind::Eof) {
                        break;
                    }
                }
            }
            let end = parser.expect_simple(&TokenKind::RBracket);
            Expr {
                span: Span::merge(token.span, end),
                kind: ExprKind::ArrayLit(ArrayLitExpr { elements }),
            }
        }
        TokenKind::LParen => {
            parser.bump(); // `(`
            // Parens lift the NO_STRUCT_LITERAL ban for their contents:
            // `(Point { x: 1 } == p)` stays unambiguous.
            let inner = parser.with_paren_escape(|parser| expr(parser));
            // A poisoned subtree means recovery already consumed (or
            // discarded) the group's tail; demanding a closer here would
            // only add cascade noise on top of the budget diagnostic.
            if matches!(inner.kind, ExprKind::Invalid)
                && !matches!(parser.peek().kind, TokenKind::RParen)
            {
                return Expr {
                    kind: ExprKind::Invalid,
                    span: token.span,
                };
            }
            let end = parser.expect_simple(&TokenKind::RParen);
            Expr {
                kind: inner.kind,
                span: Span::merge(token.span, end),
            }
        }
        _ => {
            // Recovery placeholder is poisoned, never valid-looking code.
            parser
                .diagnostics
                .push(error::expected("an expression", &token));
            parser.bump();
            Expr {
                kind: ExprKind::Invalid,
                span: token.span,
            }
        }
    }
}

/// `Name { field: expr, ... }` — struct literal (§9.2). The QualifiedName
/// head has already been consumed by the caller (a bare Ident is just the
/// len-1 case). All fields of the type must be given exactly once (checked
/// in the type checker, not here).
fn struct_lit(parser: &mut Parser, path: Vec<kai_ast::Ident>) -> Expr {
    let start_span = path[0].span;
    parser.bump(); // `{`

    let mut fields = Vec::new();
    while !matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
        let field_name = parser.expect_ident("a field name");
        parser.expect_simple(&TokenKind::Colon);
        let value = expr(parser);
        fields.push(FieldInit {
            name: field_name,
            value,
        });

        if !parser.eat_simple(&TokenKind::Comma) {
            break;
        }
        // Tolerate one trailing comma before `}`.
        if matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            break;
        }
    }

    let end = parser.expect_simple(&TokenKind::RBrace);
    Expr {
        span: Span::merge(start_span, end),
        kind: ExprKind::StructLit(StructLitExpr { path, fields }),
    }
}
