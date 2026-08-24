use kai_diagnostics::{Diagnostic, Span};
use crate::token::TokenKind;
use super::Lexer;

impl<'src> Lexer<'src> {
    /// Integer or float literal; `first` is the leading digit already
    /// consumed by the scan loop.
    pub(super) fn scan_number(&mut self, first: u8, start: usize) -> crate::token::Token {
        let mut overflowed = false;
        let int_part = self.accumulate_int(u64::from(first - b'0'), &mut overflowed);
        // Float requires at least one digit after '.' (EBNF: "1." is invalid).
        match self.cursor.peek() {
            Some(b'.')
                if self
                    .cursor
                    .peek_second()
                    .is_some_and(|b| b.is_ascii_digit()) =>
            {
                self.cursor.bump(); // '.'
                let fraction = self.accumulate_int(0, &mut overflowed);
                let mut scale = 1.0f64;
                let mut remaining = fraction;
                while remaining > 0 {
                    scale /= 10.0;
                    remaining /= 10;
                }
                let value = int_part as f64 + fraction as f64 * scale;
                return self.token(TokenKind::FloatLit(value), start);
            }
            Some(b'.') => {
                while self.cursor.peek() == Some(b'.') {
                    self.cursor.bump();
                }
                self.diagnostics.push(Diagnostic::error(
                    "float literal needs a digit after `.`",
                    Span::new(start, self.cursor.pos()),
                ));
            }
            _ => {}
        }
        if overflowed {
            self.report_int_overflow(start);
        }
        self.token(TokenKind::IntLit(int_part), start)
    }

    /// Accumulates trailing digits into `base`, saturating at `u64::MAX`.
    /// Saturation is reported by the caller so the span covers the whole
    /// literal and floats don't double-report.
    pub(super) fn accumulate_int(&mut self, base: u64, overflowed: &mut bool) -> u64 {
        let mut value = base;
        while let Some(digit) = self.cursor.eat_if(|b| b.is_ascii_digit()) {
            value = match value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(digit - b'0')))
            {
                Some(next) => next,
                None => {
                    *overflowed = true;
                    u64::MAX
                }
            };
        }
        value
    }
}
