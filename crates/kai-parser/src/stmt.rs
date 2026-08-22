//! Statements: bindings, assignment, if/else, return, bare expressions.

use crate::error;
use crate::expr;
use crate::parser::Parser;
use crate::ty;
use kai_ast::{AssignOp, AssignStmt, AssignTarget, Block, Expr, IfStmt, LetStmt, Stmt, StmtKind};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub fn block(parser: &mut Parser) -> Block {
    let start = parser.span_here();
    parser.expect_simple(&TokenKind::LBrace);

    let mut stmts = Vec::new();
    loop {
        match parser.peek().kind.clone() {
            TokenKind::RBrace | TokenKind::Eof => break,
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
        TokenKind::Let | TokenKind::Var => binding(parser),
        TokenKind::If => if_stmt(parser),
        TokenKind::LBrace => bare_block(parser),
        _ => expr_or_assign(parser),
    }
}

/// A bare `{ ... }` block statement: introduces a fresh variable scope.
fn bare_block(parser: &mut Parser) -> Stmt {
    let inner = block(parser);
    let span = inner.span;
    Stmt {
        span,
        kind: StmtKind::Block(inner),
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

/// `let` / `var name [: Type]? = init ;`
fn binding(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    let mutable = parser.peek().kind == TokenKind::Var;
    parser.bump(); // `let` / `var`

    let name = parser.expect_ident("a variable name");
    let annotation = if parser.eat_simple(&TokenKind::Colon) {
        Some(ty::ty(parser))
    } else {
        None
    };

    parser.expect_simple(&TokenKind::Eq);
    let init = expr::expr(parser);
    let end = parser.expect_simple(&TokenKind::Semi);

    Stmt {
        span: Span::merge(start, end),
        kind: StmtKind::Let(LetStmt {
            name,
            ty: annotation,
            init,
            mutable,
        }),
    }
}

/// `if cond { ... } [else (if ... | { ... })]`
fn if_stmt(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `if`

    let cond = expr::expr(parser);
    let then_block = block(parser);

    let else_block = if parser.eat_simple(&TokenKind::Else) {
        if parser.peek().kind == TokenKind::If {
            // `else if` nests as a single-statement block.
            let nested = if_stmt(parser);
            let span = nested.span;
            Some(Block {
                stmts: vec![nested],
                span,
            })
        } else {
            Some(block(parser))
        }
    } else {
        None
    };

    let span = Span::merge(
        start,
        else_block.as_ref().map_or(then_block.span, |b| b.span),
    );
    Stmt {
        span,
        kind: StmtKind::If(IfStmt {
            cond,
            then_block,
            else_block,
        }),
    }
}

/// Distinguishes assignment (`x = e;`) from a bare expression statement by
/// parsing an expression and checking for a following assign-op.
fn expr_or_assign(parser: &mut Parser) -> Stmt {
    let parsed = expr::expr(parser);
    let start = parsed.span;

    if let Some(op) = try_assign_op(parser) {
        let value = expr::expr(parser);
        let end = parser.expect_simple(&TokenKind::Semi);
        let span = Span::merge(start, end);

        return match place_from(&parsed) {
            Some(target) => Stmt {
                span,
                kind: StmtKind::Assign(AssignStmt {
                    target,
                    op,
                    value,
                    span,
                }),
            },
            None => {
                parser
                    .diagnostics
                    .push(error::custom("invalid assignment target", start));
                Stmt {
                    span,
                    kind: StmtKind::Expr(parsed),
                }
            }
        };
    }

    let end = parser.expect_simple(&TokenKind::Semi);
    Stmt {
        kind: StmtKind::Expr(parsed),
        span: Span::merge(start, end),
    }
}

fn try_assign_op(parser: &mut Parser) -> Option<AssignOp> {
    let op = match parser.peek().kind.clone() {
        TokenKind::Eq => AssignOp::Eq,
        TokenKind::PlusEq => AssignOp::PlusEq,
        TokenKind::MinusEq => AssignOp::MinusEq,
        TokenKind::StarEq => AssignOp::StarEq,
        TokenKind::SlashEq => AssignOp::SlashEq,
        _ => return None,
    };
    parser.bump();
    Some(op)
}

fn place_from(expr: &Expr) -> Option<AssignTarget> {
    match &expr.kind {
        kai_ast::ExprKind::Ident(ident) => Some(AssignTarget::Named(ident.clone())),
        _ => None,
    }
}
