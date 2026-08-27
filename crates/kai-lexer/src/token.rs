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
    For,
    In,
    While,
    // v0.0.6 keywords. `Some`/`None` construct an Optional value (§9.9a);
    // `Ok`/`Err` construct a Result value (§3.4, v0.14); `catch` is the Result error-branch postfix operator. `Result` and
    // `Optional` stay plain identifiers — they only ever name types, so
    // they resolve like `int32` does.
    SomeKw,
    NoneKw,
    OkKw,
    ErrKw,
    Catch,
    // v0.0.9 keywords (§5.3): `reversible` marks a function whose Place
    // mutations are transactionally reversible; `compensate` is the postfix
    // operator that attaches an external-effect compensation block to a call.
    Reversible,
    Compensate,
    /// Bare `_`, carved out of `Ident` as of v0.0.6 (§9.9b): reserved
    /// exclusively for the discard statement, never a binding name.
    Underscore,
    Ident(String),
    IntLit(u64),
    FloatLit(f64),
    /// Decoded string content (escapes already applied by the lexer).
    StrLit(String),
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
    /// Lone `|` — only valid as the `catch |err|` delimiter (v0.0.6); the
    /// parser rejects it anywhere a binary operator was expected.
    Pipe,
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
    LBracket,
    RBracket,
    // v0.0.6: `??` coalesce and the `?` of `T?` type sugar.
    QuestionQuestion,
    Question,
    // v0.0.7: temporal modifiers, DurationLit, effects
    At, // '@' for @local/@wallclock
    DurationLit { value: u64, unit: DurationUnit },
    Require,
    Observe,
    Effects,
    EscapesLocalContext,
    LocalKw,     // 'local' after '@'
    WallclockKw, // 'wallclock' after '@'
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationUnit {
    Ms,
    S,
    M,
    H,
    D,
}

impl DurationUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            DurationUnit::Ms => "ms",
            DurationUnit::S => "s",
            DurationUnit::M => "m",
            DurationUnit::H => "h",
            DurationUnit::D => "d",
        }
    }
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
            TokenKind::For => "`for`".into(),
            TokenKind::In => "`in`".into(),
            TokenKind::While => "`while`".into(),
            TokenKind::SomeKw => "`Some`".into(),
            TokenKind::NoneKw => "`None`".into(),
            TokenKind::OkKw => "`Ok`".into(),
            TokenKind::ErrKw => "`Err`".into(),
            TokenKind::Catch => "`catch`".into(),
            TokenKind::Reversible => "`reversible`".into(),
            TokenKind::Compensate => "`compensate`".into(),
            TokenKind::Underscore => "`_`".into(),
            TokenKind::True | TokenKind::False => "boolean literal".into(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::IntLit(value) => format!("integer literal `{value}`"),
            TokenKind::FloatLit(value) => format!("float literal `{value}`"),
            TokenKind::StrLit(_) => "string literal".into(),
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
            TokenKind::Pipe => "`|`".into(),
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
            TokenKind::LBracket => "`[`".into(),
            TokenKind::RBracket => "`]`".into(),
            TokenKind::QuestionQuestion => "`??`".into(),
            TokenKind::Question => "`?`".into(),
            TokenKind::At => "`@`".into(),
            TokenKind::DurationLit { value, unit } => format!("duration literal `{}{}`", value, unit.as_str()),
            TokenKind::Require => "`require`".into(),
            TokenKind::Observe => "`observe`".into(),
            TokenKind::Effects => "`effects`".into(),
            TokenKind::EscapesLocalContext => "`escapes-local-context`".into(),
            TokenKind::LocalKw => "`local`".into(),
            TokenKind::WallclockKw => "`wallclock`".into(),
            TokenKind::Eof => "end of file".into(),
        }
    }
}
