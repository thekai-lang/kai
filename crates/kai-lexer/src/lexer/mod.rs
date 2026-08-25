use crate::cursor::Cursor;
use crate::keywords;
use crate::operators;
use crate::token::{Token, TokenKind};
mod string;
mod number;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod v0005_tests;
#[cfg(test)]
mod v0006_tests;
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
            b',' => Some(self.token(TokenKind::Comma, start)),
            b':' => Some(self.token(TokenKind::Colon, start)),
            b'[' => Some(self.token(TokenKind::LBracket, start)),
            b']' => Some(self.token(TokenKind::RBracket, start)),
            b'"' => self.scan_string(start),
            b'@' => Some(self.token(TokenKind::At, start)),
            b'-' => Some(self.scan_minus(start)),
            b'0'..=b'9' => Some(self.scan_number(byte, start)),
            b'a'..=b'z' | b'A'..=b'Z' => self.scan_word(start),
            b'_' => {
                // v0.0.6 (§9.9b): a bare `_` lexes as its own token, reserved
                // for the discard statement. `_foo` and friends stay ordinary
                // identifiers.
                if self.cursor.peek().is_some_and(is_word_continue) {
                    self.scan_word(start)
                } else {
                    Some(self.token(TokenKind::Underscore, start))
                }
            }
            b'.' => {
                // `.5` and friends: numbers must start with a digit, so a dot
                // directly followed by one is a malformed number (recovery
                // region, one diagnostic). Otherwise it is field access.
                if self.cursor.peek().is_some_and(|d| d.is_ascii_digit()) {
                    loop {
                        match self.cursor.peek() {
                            Some(b'.') => {
                                self.cursor.bump();
                            }
                            Some(d) if d.is_ascii_digit() => {
                                self.cursor.bump();
                            }
                            _ => break,
                        }
                    }
                    self.diagnostics.push(Diagnostic::error(
                        "number literals must start with a digit",
                        Span::new(start, self.cursor.pos()),
                    ));
                    None
                } else {
                    Some(self.token(TokenKind::Dot, start))
                }
            }
            _ => match operators::scan(&mut self.cursor, byte) {
                Some(Ok(kind)) => Some(self.token(kind, start)),
                Some(Err(expected)) => {
                    let found = byte as char;
                    self.diagnostics.push(Diagnostic::error(
                        format!(
                            "unexpected character `{found}` (did you mean `{expected}{expected}`?)"
                        ),
                        Span::new(start, self.cursor.pos()),
                    ));
                    None
                }
                None => {
                    self.diagnostics.push(Diagnostic::error(
                        format!("unexpected character `{}`", byte as char),
                        Span::new(start, start + 1),
                    ));
                    None
                }
            },
        }
    }

    /// String literal (v0.0.5): `"..."` with exactly six escape sequences
    /// (`\n`, `\t`, `\r`, `\"`, `\\`, `\0`; §9.7). Anything else after a
    /// backslash is a lex error — never silent pass-through. `${` is plain
    /// text until interpolation is actually designed; newlines may not
    /// appear inside a literal (an unterminated string reports once).
    fn scan_minus(&mut self, start: usize) -> Token {
        let kind = match self.cursor.peek() {
            Some(b'>') => {
                self.cursor.bump();
                TokenKind::Arrow
            }
            Some(b'=') => {
                self.cursor.bump();
                TokenKind::MinusEq
            }
            _ => TokenKind::Minus,
        };
        self.token(kind, start)
    }

    /// Integer or float literal; `first` is the leading digit already
    /// consumed by the scan loop.
    fn scan_word(&mut self, start: usize) -> Option<Token> {
        while self.cursor.eat_if(is_word_continue).is_some() {}
        // Special handling for hyphenated keyword `escapes-local-context` (§5.1.2, EBNF §9)
        // `is_word_continue` stops at `-`, so `escapes` would be split. Check for `-local-context` suffix and consume as one token.
        let mut end = self.cursor.pos();
        let mut word = std::str::from_utf8(self.cursor.slice(start, end))
            .expect("internal error: non-ascii word slice — compiler bug")
            .to_owned();
        if word == "escapes" && self.cursor.peek() == Some(b'-') {
            // Peek ahead for `-local-context` (15 chars: `-local-context`)
            let suffix = "-local-context";
            let mut matches = true;
            for (i, ch) in suffix.bytes().enumerate() {
                if self.cursor.peek_n(i) != Some(ch) {
                    matches = false;
                    break;
                }
            }
            if matches {
                // Consume `-local-context` (15 bytes inc leading `-`)
                for _ in 0..suffix.len() {
                    self.cursor.bump();
                }
                end = self.cursor.pos();
                word = std::str::from_utf8(self.cursor.slice(start, end))
                    .expect("internal error: non-ascii word slice — compiler bug")
                    .to_owned();
            }
        }
        let kind = keywords::lookup(&word).unwrap_or_else(|| TokenKind::Ident(word.clone()));
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

