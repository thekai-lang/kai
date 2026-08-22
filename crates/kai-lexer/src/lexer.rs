use crate::cursor::Cursor;
use crate::keywords;
use crate::token::{Token, TokenKind};
use kai_diagnostics::{Diagnostic, Span};

/// Lexical output: tokens always include a trailing `Eof`; `diagnostics` holds
/// every recoverable lexical error encountered (the token stream stays
/// well-formed so downstream phases can be attempted once diags are fixed).
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(source: &str) -> LexOutput {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.run();
    LexOutput {
        tokens,
        diagnostics: lexer.diagnostics,
    }
}

struct Lexer<'src> {
    cursor: Cursor<'src>,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Lexer<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            cursor: Cursor::new(source),
            diagnostics: Vec::new(),
        }
    }

    fn run(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            let start = self.cursor.pos();
            match self.cursor.bump() {
                None => {
                    tokens.push(self.token(TokenKind::Eof, start));
                    return tokens;
                }
                Some(byte) => {
                    if let Some(token) = self.scan_byte(byte, start) {
                        tokens.push(token);
                    }
                }
            }
        }
    }

    /// Scans one non-trivia byte into a token. Returns `None` after recording
    /// a diagnostic for bytes that cannot start any token.
    fn scan_byte(&mut self, byte: u8, start: usize) -> Option<Token> {
        match byte {
            b'(' => Some(self.token(TokenKind::LParen, start)),
            b')' => Some(self.token(TokenKind::RParen, start)),
            b'{' => Some(self.token(TokenKind::LBrace, start)),
            b'}' => Some(self.token(TokenKind::RBrace, start)),
            b';' => Some(self.token(TokenKind::Semi, start)),
            b'-' if self.cursor.peek() == Some(b'>') => {
                self.cursor.bump();
                Some(self.token(TokenKind::Arrow, start))
            }
            b'0'..=b'9' => Some(self.scan_int(byte, start)),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.scan_word(start),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    format!("unexpected character `{}`", byte as char),
                    Span::new(start, start + 1),
                ));
                None
            }
        }
    }

    /// `first` is the leading digit already consumed by the scan loop; it
    /// counts toward the value.
    fn scan_int(&mut self, first: u8, start: usize) -> Token {
        let mut overflowed = false;
        let mut value = u64::from(first - b'0');

        while let Some(digit) = self.cursor.eat_if(|b| b.is_ascii_digit()) {
            value = match value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(digit - b'0')))
            {
                Some(next) => next,
                None => {
                    overflowed = true;
                    u64::MAX
                }
            };
        }

        if overflowed {
            self.report_int_overflow(start);
        }

        self.token(TokenKind::IntLit(value), start)
    }

    fn scan_word(&mut self, start: usize) -> Option<Token> {
        while self.cursor.eat_if(is_word_continue).is_some() {}
        let end = self.cursor.pos();
        // Slice is safe: the word is ASCII by construction.
        let word = std::str::from_utf8(self.cursor.slice(start, end)).expect("ascii word");
        let kind = keywords::lookup(word).unwrap_or_else(|| TokenKind::Ident(word.to_owned()));
        Some(self.token(kind, start))
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.cursor.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => {
                    self.cursor.bump();
                }
                Some(b'/') if self.cursor.peek_second() == Some(b'/') => {
                    while self.cursor.eat_if(|b| b != b'\n').is_some() {}
                }
                _ => return,
            }
        }
    }

    fn report_int_overflow(&mut self, start: usize) {
        let already = matches!(self.diagnostics.last(), Some(d) if d.span.start == start);
        if !already {
            self.diagnostics.push(Diagnostic::error(
                "integer literal is too large",
                Span::new(start, self.cursor.pos()),
            ));
        }
    }

    fn token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            span: Span::new(start, self.cursor.pos()),
        }
    }
}

fn is_word_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_diagnostics::SourceMap;

    const MINIMAL: &str = "fn main() -> int32 {\n    return 0;\n}\n";

    fn kinds(tokens: &[Token]) -> Vec<&TokenKind> {
        tokens.iter().map(|t| &t.kind).collect()
    }

    #[test]
    fn lexes_minimal_program() {
        let out = lex(MINIMAL);
        assert!(out.diagnostics.is_empty());
        assert_eq!(
            kinds(&out.tokens),
            vec![
                &TokenKind::Fn,
                &TokenKind::Ident("main".into()),
                &TokenKind::LParen,
                &TokenKind::RParen,
                &TokenKind::Arrow,
                &TokenKind::Ident("int32".into()),
                &TokenKind::LBrace,
                &TokenKind::Return,
                &TokenKind::IntLit(0),
                &TokenKind::Semi,
                &TokenKind::RBrace,
                &TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn spans_point_at_correct_positions() {
        let out = lex(MINIMAL);
        let ret_token = out
            .tokens
            .iter()
            .find(|t| t.kind == TokenKind::Return)
            .unwrap();
        let map = SourceMap::new(MINIMAL);
        let lc = map.line_col(ret_token.span.start);
        assert_eq!((lc.line, lc.col), (2, 5));
    }

    #[test]
    fn skips_line_comments() {
        let out = lex("// hello\nfn main() {}");
        assert!(out.diagnostics.is_empty());
        assert_eq!(kinds(&out.tokens).len(), 7); // fn ident ( ) { } Eof
    }

    #[test]
    fn reports_unknown_character_and_continues() {
        let out = lex("fn @ main");
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].message, "unexpected character `@`");
        assert!(out.tokens.iter().any(|t| t.kind == TokenKind::Fn));
    }

    #[test]
    fn reports_integer_overflow_once() {
        let big = format!("{}9", u64::MAX);
        let out = lex(&format!("return {big};"));
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].message, "integer literal is too large");
    }

    #[test]
    fn multi_digit_literal_keeps_every_digit() {
        let out = lex("return 2147483648;");
        let lit = out
            .tokens
            .iter()
            .find_map(|t| match t.kind {
                TokenKind::IntLit(v) => Some(v),
                _ => None,
            })
            .expect("literal token");
        assert_eq!(lit, 2_147_483_648);
    }

    #[test]
    fn bare_minus_without_arrow_is_error() {
        let out = lex("return 0 - 1;");
        // `-` followed by space is not `->`; v0.0.1 has no subtraction yet.
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].message, "unexpected character `-`");
    }
}
