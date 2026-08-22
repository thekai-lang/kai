use crate::error;
use kai_ast::{Expr, ExprKind};
use kai_diagnostics::{Diagnostic, Span};
use kai_lexer::{Token, TokenKind};

/// Recursion budget for expression parsing. Every potentially recursive
/// production funnels through `expr()`, so one counter bounds the AST depth
/// no matter how nesting is written (parens today; calls/postfix later).
pub(crate) const MAX_EXPR_DEPTH: u32 = 256;

/// Token-stream cursor shared by all parsing submodules. Owns the diagnostic
/// list; submodule parsers push errors and recover locally.
pub struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    depth: u32,
    /// Scoped to a single overflow event: reset once its recovery region has
    /// been skipped, so later independent deep expressions report too.
    pub(crate) depth_reported: bool,
    /// Rust's NO_STRUCT_LITERAL rule (§9.3): inside an `if` condition a bare
    /// `Ident {` never starts a struct literal — the `{` belongs to the block.
    /// Cleared again inside parentheses, so `(Point { x: 1 } == p)` works.
    struct_lit_banned: bool,
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            depth: 0,
            depth_reported: false,
            struct_lit_banned: false,
        }
    }

    /// Parses `body` with struct literals banned (`if` conditions). Restores
    /// the previous state afterwards, including for `else if` chains.
    pub(crate) fn with_struct_lits_banned(&mut self, body: impl FnOnce(&mut Self) -> Expr) -> Expr {
        let saved = std::mem::replace(&mut self.struct_lit_banned, true);
        let out = body(self);
        self.struct_lit_banned = saved;
        out
    }

    pub(crate) fn struct_lits_banned(&self) -> bool {
        self.struct_lit_banned
    }

    /// Parses `body` inside explicit parens, where struct literals are always
    /// allowed regardless of the surrounding ban state.
    pub(crate) fn with_paren_escape(&mut self, body: impl FnOnce(&mut Self) -> Expr) -> Expr {
        let saved = std::mem::replace(&mut self.struct_lit_banned, false);
        let out = body(self);
        self.struct_lit_banned = saved;
        out
    }

    /// Enters an expression-production recursion level. `false` = budget
    /// exhausted; the counter is left untouched and the caller must recover
    /// without recursing further.
    pub(crate) fn enter_expr(&mut self) -> bool {
        if self.depth >= MAX_EXPR_DEPTH {
            if !self.depth_reported {
                self.depth_reported = true;
                let span = self.span_here();
                self.diagnostics.push(error::expression_too_deep(span));
            }
            return false;
        }
        self.depth += 1;
        true
    }

    pub(crate) fn exit_expr(&mut self) {
        self.depth -= 1;
    }

    /// Runs `body` under budget accounting. Over-budget invocations skip the
    /// enclosing paren group iteratively and yield `ExprKind::Invalid`.
    pub(crate) fn guarded_expr(&mut self, body: impl FnOnce(&mut Self) -> Expr) -> Expr {
        if !self.enter_expr() {
            let span = self.skip_to_group_end();
            // Recovery region closed: future deep expressions report again.
            self.depth_reported = false;
            return Expr {
                kind: ExprKind::Invalid,
                span,
            };
        }
        let out = body(self);
        self.exit_expr();
        out
    }

    /// Iteratively advances past the token range belonging to the paren group
    /// whose contents will not be parsed (budget recovery). Stops after the
    /// `)` that closes the innermost unclosed group; at nesting level zero it
    /// stops BEFORE statement boundaries (`;`, `}`) so the enclosing grammar
    /// keeps its own delimiters. EOF-safe.
    pub(crate) fn skip_to_group_end(&mut self) -> Span {
        let start = self.span_here();
        let mut balance: i32 = 0;
        let mut end = start;
        while !self.at_eof() {
            match self.peek().kind {
                TokenKind::LParen => balance += 1,
                TokenKind::RParen => {
                    balance -= 1;
                    end = self.span_here();
                    self.bump();
                    if balance < 0 {
                        return Span::merge(start, end);
                    }
                    continue;
                }
                // Statement boundary: leave it for the caller's grammar.
                TokenKind::Semi | TokenKind::RBrace if balance == 0 => {
                    return Span::merge(start, end);
                }
                _ => {}
            }
            end = self.span_here();
            self.bump();
        }
        Span::merge(start, end)
    }

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    pub(crate) fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if !self.at_eof() {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    pub(crate) fn span_here(&self) -> Span {
        self.peek().span
    }

    /// Consumes the next token iff it is a payload-free kind.
    pub(crate) fn eat_simple(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consumes `kind` or records "expected X".
    pub(crate) fn expect_simple(&mut self, kind: &TokenKind) -> Span {
        if self.eat_simple(kind) {
            return self.tokens[self.pos - 1].span;
        }
        let found = self.peek().clone();
        self.diagnostics
            .push(error::expected(kind.describe(), &found));
        found.span
    }

    /// Consumes an identifier or records an error; the name is empty on failure.
    pub(crate) fn expect_ident(&mut self, what: &str) -> kai_ast::Ident {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(name) => {
                self.bump();
                kai_ast::Ident {
                    name,
                    span: token.span,
                }
            }
            _ => {
                self.diagnostics.push(error::expected(what, &token));
                kai_ast::Ident {
                    name: String::new(),
                    span: token.span,
                }
            }
        }
    }
}
