use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Return,
    Let,
    Var,
    If,
    Else,
    True,
    False,
    Type,
    Mut,
    Use,
    Public,
    Ident(String),
    IntLit(u64),
    FloatLit(f64),
    // Arithmetic / assignment
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    // Comparison
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    // Logic
    AmpAmp,
    PipePipe,
    Bang,
    // Punctuation
    Arrow,
    Comma,
    Dot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semi,
    Colon,
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
            TokenKind::Let => "`let`".into(),
            TokenKind::Var => "`var`".into(),
            TokenKind::If => "`if`".into(),
            TokenKind::Else => "`else`".into(),
            TokenKind::Type => "`type`".into(),
            TokenKind::Mut => "`mut`".into(),
            TokenKind::Use => "`use`".into(),
            TokenKind::Public => "`public`".into(),
            TokenKind::True | TokenKind::False => "boolean literal".into(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::IntLit(value) => format!("integer literal `{value}`"),
            TokenKind::FloatLit(value) => format!("float literal `{value}`"),
            TokenKind::Plus => "`+`".into(),
            TokenKind::Minus => "`-`".into(),
            TokenKind::Star => "`*`".into(),
            TokenKind::Slash => "`/`".into(),
            TokenKind::Percent => "`%`".into(),
            TokenKind::Eq => "`=`".into(),
            TokenKind::PlusEq => "`+=`".into(),
            TokenKind::MinusEq => "`-=`".into(),
            TokenKind::StarEq => "`*=`".into(),
            TokenKind::SlashEq => "`/=`".into(),
            TokenKind::EqEq => "`==`".into(),
            TokenKind::NotEq => "`!=`".into(),
            TokenKind::Lt => "`<`".into(),
            TokenKind::Gt => "`>`".into(),
            TokenKind::Le => "`<=`".into(),
            TokenKind::Ge => "`>=`".into(),
            TokenKind::AmpAmp => "`&&`".into(),
            TokenKind::PipePipe => "`||`".into(),
            TokenKind::Bang => "`!`".into(),
            TokenKind::Arrow => "`->`".into(),
            TokenKind::Comma => "`,`".into(),
            TokenKind::Dot => "`.`".into(),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::LBrace => "`{`".into(),
            TokenKind::RBrace => "`}`".into(),
            TokenKind::Semi => "`;`".into(),
            TokenKind::Colon => "`:`".into(),
            TokenKind::Eof => "end of file".into(),
        }
    }
}
