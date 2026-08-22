//! Expressions: precedence-climbing chain, highest binding last.
//! Chain (loosest → tightest): || → && → ?? → ==/!= → relational → +- → */% → unary → primary.
//! Assignment is NOT part of the expression grammar (statement-only).

use crate::error;
use crate::parser::Parser;
use kai_ast::{
    BinaryExpr, BinaryOp, CallExpr, Expr, ExprKind, FieldAccessExpr, FieldInit, FloatLit, IntLit,
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

/// `??` is right-associative; it arrives with Optionals (v0.0.5). The level
/// exists in the chain so its position is explicit.
fn coalesce(parser: &mut Parser) -> Expr {
    equality(parser)
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
        TokenKind::Ident(name) => {
            // Consume the identifier first, then decide what follows it:
            // `{` opens a struct literal (§9.2) unless we are inside an if
            // condition — there the NO_STRUCT_LITERAL rule (§9.3) leaves the
            // brace to the statement grammar as a block.
            parser.bump();
            if parser.peek().kind == TokenKind::LBrace && !parser.struct_lits_banned() {
                return struct_lit(parser, name, token.span);
            }
            Expr {
                kind: ExprKind::Ident(kai_ast::Ident {
                    name,
                    span: token.span,
                }),
                span: token.span,
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

/// `Name { field: expr, ... }` — struct literal (§9.2). The identifier has
/// already been consumed by the caller; all fields of the type must be given
/// exactly once (checked in the type checker, not here).
fn struct_lit(parser: &mut Parser, name: String, name_span: Span) -> Expr {
    let ident = kai_ast::Ident {
        name,
        span: name_span,
    };
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
        span: Span::merge(name_span, end),
        kind: ExprKind::StructLit(StructLitExpr {
            name: ident,
            fields,
        }),
    }
}
