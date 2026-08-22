//! Expressions: precedence-climbing chain, highest binding last.
//! Chain (loosest → tightest): || → && → ?? → ==/!= → relational → +- → */% → unary → primary.
//! Assignment is NOT part of the expression grammar (statement-only).

use crate::error;
use crate::parser::Parser;
use kai_ast::{BinaryExpr, BinaryOp, Expr, ExprKind, FloatLit, IntLit, UnaryExpr, UnaryOp};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn expr(parser: &mut Parser) -> Expr {
    logic_or(parser)
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

fn unary(parser: &mut Parser) -> Expr {
    let op = match parser.peek().kind.clone() {
        TokenKind::Minus => Some(UnaryOp::Neg),
        TokenKind::Bang => Some(UnaryOp::Not),
        _ => None,
    };

    match op {
        Some(op) => {
            let op_span = parser.span_here();
            parser.bump();
            let operand = unary(parser);
            let span = Span::merge(op_span, operand.span);
            Expr {
                span,
                kind: ExprKind::Unary(UnaryExpr {
                    op,
                    op_span,
                    operand: Box::new(operand),
                }),
            }
        }
        None => postfix(parser),
    }
}

fn postfix(parser: &mut Parser) -> Expr {
    primary(parser)
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
            leaf!(ExprKind::Ident(kai_ast::Ident {
                name,
                span: token.span
            }))
        }
        TokenKind::LParen => {
            parser.bump(); // `(`
            let inner = expr(parser);
            let end = parser.expect_simple(&TokenKind::RParen);
            Expr {
                kind: inner.kind,
                span: Span::merge(token.span, end),
            }
        }
        _ => {
            parser
                .diagnostics
                .push(error::expected("an expression", &token));
            parser.bump();
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
