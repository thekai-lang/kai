use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Return,
    Ident(String),
    IntLit(u64),
    Arrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semi,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Human-readable form for "expected X, found Y" diagnostics.
    pub fn describe(&self) -> String {
        self.kind.describe()
    }
}

impl TokenKind {
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Fn => "`fn`".into(),
            TokenKind::Return => "`return`".into(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::IntLit(value) => format!("integer literal `{value}`"),
            TokenKind::Arrow => "`->`".into(),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::LBrace => "`{`".into(),
            TokenKind::RBrace => "`}`".into(),
            TokenKind::Semi => "`;`".into(),
            TokenKind::Eof => "end of file".into(),
        }
    }
}
