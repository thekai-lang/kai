use kai_diagnostics::{Diagnostic, Span};
use crate::token::TokenKind;
use super::Lexer;

impl<'src> Lexer<'src> {
    /// String literal (v0.0.5): `"..."` with exactly six escape sequences
    /// (`\n`, `\t`, `\r`, `\"`, `\\`, `\0`; §9.7). Anything else after a
    /// backslash is a lex error — never silent pass-through. `${` is plain
    /// text until interpolation is actually designed; newlines may not
    /// appear inside a literal (an unterminated string reports once).
    pub(super) fn scan_string(&mut self, start: usize) -> Option<crate::token::Token> {
        let mut content: Vec<u8> = Vec::new();
        loop {
            match self.cursor.bump() {
                None | Some(b'\n') => {
                    self.diagnostics.push(Diagnostic::error(
                        "unterminated string literal",
                        Span::new(start, self.cursor.pos()),
                    ));
                    return None;
                }
                Some(b'"') => break,
                Some(b'\\') => match self.cursor.bump() {
                    Some(b'n') => content.push(b'\n'),
                    Some(b't') => content.push(b'\t'),
                    Some(b'r') => content.push(b'\r'),
                    Some(b'"') => content.push(b'"'),
                    Some(b'\\') => content.push(b'\\'),
                    Some(b'0') => content.push(0),
                    Some(other) => {
                        content.push(other);
                        self.diagnostics.push(Diagnostic::error(
                            format!(
                                "unknown escape sequence `\\{}` (expected one of n, t, r, 0, quote, backslash)",
                                other as char
                            ),
                            Span::new(self.cursor.pos() - 2, self.cursor.pos()),
                        ));
                    }
                    None => {
                        self.diagnostics.push(Diagnostic::error(
                            "unterminated string literal",
                            Span::new(start, self.cursor.pos()),
                        ));
                        return None;
                    }
                },
                Some(byte) => content.push(byte),
            }
        }
        let text = String::from_utf8_lossy(&content).into_owned();
        Some(self.token(TokenKind::StrLit(text), start))
    }
}
