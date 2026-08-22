use crate::error;
use kai_diagnostics::{Diagnostic, Span};
use kai_lexer::{Token, TokenKind};

/// Token-stream cursor shared by all parsing submodules. Owns the diagnostic
/// list; submodule parsers push errors and recover locally.
pub struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
        }
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
