//! Statements: bindings, assignment, if/else, for..in, return, bare expressions.

use crate::error;
use crate::expr;
use crate::parser::Parser;
use crate::ty;
use kai_ast::{
    AssignOp, AssignStmt, AssignTarget, Block, Expr, ExprKind, ForStmt, IfStmt, LetStmt,
    PlaceStep, Stmt, StmtKind,
};
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
        TokenKind::For => for_stmt(parser),
        TokenKind::LBrace => bare_block(parser),
        // `_ = expr;` (v0.0.6, §9.9b) — `_` cannot start an expression, so
        // this dispatch is unambiguous. Note `let _ = ...` needs no special
        // case: binding() demands an Ident and finds Underscore instead.
        TokenKind::Underscore => discard(parser),
        TokenKind::Require => require_stmt(parser),
        TokenKind::Observe => observe_stmt(parser),
        _ => expr_or_assign(parser),
    }
}

fn require_stmt(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `require`
    let expr = expr::expr(parser);
    let end = parser.expect_simple(&TokenKind::Semi);
    Stmt {
        span: Span::merge(start, end),
        kind: StmtKind::Require(expr),
    }
}

fn observe_stmt(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `observe`
    let expr = expr::expr(parser);
    let end = parser.expect_simple(&TokenKind::Semi);
    Stmt {
        span: Span::merge(start, end),
        kind: StmtKind::Observe(expr),
    }
}

/// `_ = expr;` — the sole explicit-discard statement (§9.9b). The expression
/// evaluates normally; only the binding is skipped. Typecheck decides whether
/// the discarded type needed this escape hatch (Optional/Result diagnostics).
fn discard(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `_`
    parser.expect_simple(&TokenKind::Eq);
    let value = expr::expr(parser);
    let end = parser.expect_simple(&TokenKind::Semi);
    Stmt {
        span: Span::merge(start, end),
        kind: StmtKind::Discard(value),
    }
}

/// `'{' { Stmt } Expr '}'` — CatchBlock (v0.0.6, §3.4): ordinary statements,
/// then exactly ONE mandatory trailing value expression without `;`. This
/// narrow shape exists only for `catch`; general blocks stay statement-only
/// (`Block ::= '{' { Stmt } '}'`), so block-as-expression never leaks into
/// the rest of the language.
///
/// Disambiguation: block/if/for/binding/discard start with dedicated tokens,
/// so they are unambiguous statements. Anything else is expression-shaped —
/// it is a statement iff an assign-op or `;` follows, otherwise it IS the
/// mandatory tail. Same rules as `expr_or_assign`, just two-way splitting
/// instead of demanding the semicolon up front.
pub(crate) fn catch_block(parser: &mut Parser) -> (Vec<Stmt>, Expr, Span) {
    let start = parser.span_here();
    parser.expect_simple(&TokenKind::LBrace);

    let mut stmts = Vec::new();
    let mut tail: Option<Expr> = None;
    while tail.is_none() {
        match parser.peek().kind.clone() {
            TokenKind::RBrace | TokenKind::Eof => {
                parser.diagnostics.push(error::custom(
                    "a catch block must end with a value expression",
                    parser.span_here(),
                ));
                break;
            }
            // Dedicated statement starters: never the tail.
            TokenKind::Return
            | TokenKind::Let
            | TokenKind::Var
            | TokenKind::If
            | TokenKind::For
            | TokenKind::LBrace
            | TokenKind::Require
            | TokenKind::Observe => {
                let s = stmt(parser);
                stmts.push(s);
            }
            TokenKind::Underscore => {
                let s = discard(parser);
                stmts.push(s);
            }
            _ => {
                // Expression-shaped: statement (assign-op or `;` follows) or tail.
                let first = expr::expr(parser);
                if let Some(op) = try_assign_op(parser) {
                    let start_span = first.span;
                    let value = expr::expr(parser);
                    let end = parser.expect_simple(&TokenKind::Semi);
                    let span = Span::merge(start_span, end);
                    let assigned = match place_from(&first) {
                        Some(target) => target,
                        None => {
                            parser
                                .diagnostics
                                .push(error::custom("invalid assignment target", start_span));
                            continue;
                        }
                    };
                    stmts.push(Stmt {
                        span,
                        kind: StmtKind::Assign(AssignStmt {
                            target: assigned,
                            op,
                            value,
                            span,
                        }),
                    });
                    continue;
                }
                if parser.eat_simple(&TokenKind::Semi) {
                    // Ordinary expression statement — same call-only rule as
                    // the top-level statement grammar (§6).
                    let start_span = first.span;
                    if !matches!(first.kind, ExprKind::Call(_) | ExprKind::Invalid) {
                        parser.diagnostics.push(error::custom(
                            "only function calls can appear as expression statements",
                            start_span,
                        ));
                    }
                    stmts.push(Stmt {
                        kind: StmtKind::Expr(first),
                        span: start_span,
                    });
                    continue;
                }
                // No operator, no semicolon: this is the mandatory tail.
                if parser.peek().kind != TokenKind::RBrace && !parser.at_eof() {
                    parser.diagnostics.push(error::custom(
                        "a catch block must end with a value expression",
                        first.span,
                    ));
                }
                tail = Some(first);
            }
        }
    }

    let end = parser.expect_simple(&TokenKind::RBrace);
    let tail = tail.unwrap_or(Expr {
        kind: ExprKind::Invalid,
        span: end,
    });
    (stmts, tail, Span::merge(start, end))
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
///
/// The condition parses under the NO_STRUCT_LITERAL rule (§9.3): a bare
/// `Ident {` never opens a struct literal here, so the `{` that follows is
/// always the block.
fn if_stmt(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `if`

    let cond = parser.with_struct_lits_banned(|parser| expr::expr(parser));
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

/// `for name in expr { ... }` (v0.0.5, §9.9): iterates an array, borrowing
/// each element into `name` for one iteration.
///
/// The iterable parses under NO_STRUCT_LITERAL (§9.3) exactly like an `if`
/// condition: in `for c in chars {`, that `{` is always the loop body.
fn for_stmt(parser: &mut Parser) -> Stmt {
    let start = parser.span_here();
    parser.bump(); // `for`

    let binding = parser.expect_ident("a loop variable name");
    parser.expect_simple(&TokenKind::In);
    let iterable = parser.with_struct_lits_banned(|parser| expr::expr(parser));
    let body = block(parser);

    let span = Span::merge(start, body.span);
    Stmt {
        span,
        kind: StmtKind::For(ForStmt {
            binding,
            iterable,
            body,
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
    // EBNF §6 (v0.0.3): `ExprStmt ::= CallExprStmt` — bare expression
    // statements must be calls. `Invalid` stays silent: it already reported.
    if !matches!(parsed.kind, ExprKind::Call(_) | ExprKind::Invalid) {
        parser.diagnostics.push(error::custom(
            "only function calls can appear as expression statements",
            start,
        ));
    }
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

/// Accepts any `Place` (EBNF §6, v0.0.5): a bare identifier or a path of
/// field/index projections rooted at one identifier (`p.x`, `arr[0]`,
/// `p.arr[i].y`). Everything else — literals, calls, parenthesized exprs,
/// operators — is not assignable. The ROOT alone decides writability (§9.3):
/// every projection inherits it uniformly.
fn place_from(expr: &Expr) -> Option<AssignTarget> {
    let mut steps_rev = Vec::new();
    let mut cursor = expr;
    loop {
        match &cursor.kind {
            kai_ast::ExprKind::Ident(root) => {
                if steps_rev.is_empty() {
                    return Some(AssignTarget::Named(root.clone()));
                }
                steps_rev.reverse();
                return Some(AssignTarget::Path {
                    root: root.clone(),
                    steps: steps_rev,
                });
            }
            kai_ast::ExprKind::FieldAccess(access) => {
                steps_rev.push(PlaceStep::Field(access.field.clone()));
                cursor = access.base.as_ref();
            }
            kai_ast::ExprKind::Index(indexed) => {
                steps_rev.push(PlaceStep::Index {
                    index: indexed.index.as_ref().clone(),
                    rbracket: indexed.rbracket,
                });
                cursor = indexed.base.as_ref();
            }
            _ => return None,
        }
    }
}
