use crate::cursor::Cursor;
use crate::keywords;
use crate::operators;
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
            b',' => Some(self.token(TokenKind::Comma, start)),
            b':' => Some(self.token(TokenKind::Colon, start)),
            b'[' => Some(self.token(TokenKind::LBracket, start)),
            b']' => Some(self.token(TokenKind::RBracket, start)),
            b'"' => self.scan_string(start),
            b'-' => Some(self.scan_minus(start)),
            b'0'..=b'9' => Some(self.scan_number(byte, start)),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.scan_word(start),
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

    /// String literal (v0.0.5): `"..."` with exactly five escape sequences
    /// (`\n`, `\t`, `\r`, `\"`, `\\`; §9.7). Anything else after a
    /// backslash is a lex error — never silent pass-through. `${` is plain
    /// text until interpolation is actually designed; newlines may not
    /// appear inside a literal (an unterminated string reports once).
    fn scan_string(&mut self, start: usize) -> Option<Token> {
        let mut content: Vec<u8> = Vec::new();
        loop {
            match self.cursor.bump() {
                None | Some(b'\n') => {
                    // EOF or a raw newline both mean the closing quote never
                    // came. The span covers the whole recovery region.
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
                    Some(other) => {
                        // Keep scanning so one bad escape doesn't cascade;
                        // the offending character stays literally.
                        content.push(other);
                        self.diagnostics.push(Diagnostic::error(
                            format!(
                                "unknown escape sequence `\\{}` (expected one of n, t, r, quote, backslash)",
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

    /// `-` is three tokens deep: `->`, `-=`, or plain minus.
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
    fn scan_number(&mut self, first: u8, start: usize) -> Token {
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
            // Malformed like `1.` / `1..2`: consume the dot run as one
            // recovery region so the literal reports exactly once.
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
    fn accumulate_int(&mut self, base: u64, overflowed: &mut bool) -> u64 {
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
    fn lexes_float_literals() {
        let out = lex("let x = 3.25;");
        assert_eq!(
            kinds(&out.tokens),
            vec![
                &TokenKind::Let,
                &TokenKind::Ident("x".into()),
                &TokenKind::Eq,
                &TokenKind::FloatLit(3.25),
                &TokenKind::Semi,
                &TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn dot_without_digit_is_not_a_float() {
        // "1." must not become FloatLit; it reports a dedicated diagnostic.
        let out = lex("1.x");
        assert_eq!(out.tokens[0].kind, TokenKind::IntLit(1));
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].message.contains("needs a digit after"));
    }

    /// Numeric literal matrix: each malformed shape reports exactly one clear
    /// diagnostic; valid shapes stay silent.
    #[test]
    fn numeric_literal_matrix() {
        let cases: &[(&str, Vec<TokenKind>, usize)] = &[
            ("1", vec![TokenKind::IntLit(1)], 0),
            ("1.2", vec![TokenKind::FloatLit(1.2)], 0),
            (
                "1.",
                vec![TokenKind::IntLit(1)],
                1, // needs a digit after `.`
            ),
            (
                "1.foo",
                vec![TokenKind::IntLit(1), TokenKind::Ident("foo".into())],
                1,
            ),
            (
                "1..2",
                vec![TokenKind::IntLit(1), TokenKind::IntLit(2)],
                1, // both dots consumed as one recovery region
            ),
            (
                ".5",
                vec![], // whole run consumed as one recovery region
                1,      // must start with digit
            ),
        ];

        for (source, expected_kinds, expected_diags) in cases {
            let out = lex(source);
            assert_eq!(
                out.diagnostics.len(),
                *expected_diags,
                "diagnostics for {source:?}: {:?}",
                out.diagnostics
                    .iter()
                    .map(|d| &d.message)
                    .collect::<Vec<_>>()
            );
            let actual: Vec<&TokenKind> = out
                .tokens
                .iter()
                .filter(|t| !matches!(t.kind, TokenKind::Eof))
                .map(|t| &t.kind)
                .collect();
            let expected_refs: Vec<&TokenKind> = expected_kinds.iter().by_ref().collect();
            assert_eq!(actual, expected_refs, "tokens for {source:?}");
        }
    }

    #[test]
    fn malformed_float_messages_are_precise() {
        let out = lex("let x = 1.;");
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].message.contains("needs a digit after"));

        let out = lex("return .5;");
        assert_eq!(out.diagnostics.len(), 1);
        assert!(
            out.diagnostics[0]
                .message
                .contains("must start with a digit")
        );
    }

    #[test]
    fn disambiguates_minus_forms() {
        let out = lex("-> -= -");
        assert_eq!(
            kinds(&out.tokens),
            vec![
                &TokenKind::Arrow,
                &TokenKind::MinusEq,
                &TokenKind::Minus,
                &TokenKind::Eof
            ]
        );
    }

    #[test]
    fn lexes_logic_and_comparison_operators() {
        let out = lex("a && b || !c == 1 != 2 >= 3");
        assert!(out.diagnostics.is_empty());
        let ops: Vec<&TokenKind> = out
            .tokens
            .iter()
            .filter(|t| {
                matches!(
                    t.kind,
                    TokenKind::AmpAmp
                        | TokenKind::PipePipe
                        | TokenKind::Bang
                        | TokenKind::EqEq
                        | TokenKind::NotEq
                        | TokenKind::Ge
                )
            })
            .map(|t| &t.kind)
            .collect();
        assert_eq!(ops.len(), 6);
    }

    #[test]
    fn lone_ampersand_suggests_double() {
        let out = lex("if a & b {}");
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].message.contains("&&"));
    }
}

#[cfg(test)]
mod v0005_tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn plain_string_literal() {
        assert_eq!(
            kinds(r#"  "hello world"  "#),
            vec![TokenKind::StrLit("hello world".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn escape_sequences_decode() {
        assert_eq!(
            kinds(r#""a\nb\tc\rd\"e\\f""#),
            vec![
                TokenKind::StrLit("a\nb\tc\rd\"e\\f".into()),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn unknown_escape_is_lex_error_but_string_scans_on() {
        let out = lex(r#""a\qb""#);
        assert!(out
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unknown escape sequence")));
        // One token still produced: recovery keeps the offending char.
        match &out.tokens[0].kind {
            TokenKind::StrLit(text) => assert_eq!(text, "aqb"),
            other => panic!("expected string token, got {other:?}"),
        }
    }

    #[test]
    fn unterminated_string_reports_once() {
        let out = lex(r#"let s = "oops"#);
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].message.contains("unterminated"));
    }

    #[test]
    fn dollar_brace_is_plain_text() {
        // Interpolation is deferred (§9.7): `${` is ordinary literal text.
        let out = lex(r#""value: ${x}""#);
        assert!(out.diagnostics.is_empty());
        match &out.tokens[0].kind {
            TokenKind::StrLit(text) => assert_eq!(text, "value: ${x}"),
            other => panic!("expected string token, got {other:?}"),
        }
    }

    #[test]
    fn brackets_and_for_in_keywords() {
        let ks = kinds("for x in arr [1, 2]");
        assert!(matches!(ks[0], TokenKind::For));
        assert_eq!(ks[2], TokenKind::In);
        assert!(matches!(ks[4], TokenKind::LBracket));
        assert!(ks.last().is_some_and(|k| matches!(k, TokenKind::Eof)));
    }
}
