/// Single-byte lookahead cursor over ASCII source text (Kai's lexical grammar
/// is ASCII per the EBNF; non-ASCII bytes are reported by the lexer).
pub struct Cursor<'src> {
    src: &'src [u8],
    pos: usize,
}

impl<'src> Cursor<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    pub fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    pub fn peek_second(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    pub fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    /// Consumes and returns the next byte if it satisfies `pred`.
    pub fn eat_if(&mut self, pred: impl FnOnce(u8) -> bool) -> Option<u8> {
        match self.peek() {
            Some(byte) if pred(byte) => self.bump(),
            _ => None,
        }
    }

    /// Already-scanned bytes in `[start, end)`; both offsets must not exceed
    /// the current cursor position.
    pub(crate) fn slice(&self, start: usize, end: usize) -> &'src [u8] {
        &self.src[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_bytes_in_order() {
        let mut cur = Cursor::new("ab");
        assert_eq!(cur.bump(), Some(b'a'));
        assert_eq!(cur.pos(), 1);
        assert_eq!(cur.bump(), Some(b'b'));
        assert_eq!(cur.bump(), None);
        assert!(cur.is_eof());
    }

    #[test]
    fn peek_does_not_advance() {
        let mut cur = Cursor::new("x");
        assert_eq!(cur.peek(), Some(b'x'));
        assert_eq!(cur.eat_if(|b| b == b'x'), Some(b'x'));
        assert_eq!(cur.eat_if(|b| b == b'x'), None);
    }
}
