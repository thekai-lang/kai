use crate::token::TokenKind;

/// Reserved words. Everything else matching the identifier shape lexes as
/// `TokenKind::Ident` — including primitive type names (`int32`), which stay
/// identifiers until the type checker resolves them.
pub fn lookup(word: &str) -> Option<TokenKind> {
    match word {
        "fn" => Some(TokenKind::Fn),
        "return" => Some(TokenKind::Return),
        "let" => Some(TokenKind::Let),
        "var" => Some(TokenKind::Var),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        "true" => Some(TokenKind::True),
        "false" => Some(TokenKind::False),
        // v0.0.3 keywords. `type` opens a struct declaration; `mut` marks a
        // mutable parameter (§9.3).
        "type" => Some(TokenKind::Type),
        "mut" => Some(TokenKind::Mut),
        // v0.0.4 keywords. `use` opens a module import; `public` marks a
        // fn/type as visible through the importing module's alias (§3.6).
        "use" => Some(TokenKind::Use),
        "public" => Some(TokenKind::Public),
        // v0.0.5 keywords. `for`/`in` open the array iteration loop (§9.9).
        "for" => Some(TokenKind::For),
        "in" => Some(TokenKind::In),
        // v0.0.8.1: `while` implemented (GAP-1 closure) — condition loop.
        "while" => Some(TokenKind::While),
        // v0.0.6 keywords (§9.9a/§9.9b). `Some`/`None` construct an Optional;
        // `Ok`/`Err` construct a Result (§3.4, v0.14); `catch` is a postfix operator on Result.
        "Some" => Some(TokenKind::SomeKw),
        "None" => Some(TokenKind::NoneKw),
        "Ok" => Some(TokenKind::OkKw),
        "Err" => Some(TokenKind::ErrKw),
        "catch" => Some(TokenKind::Catch),
        // v0.0.7 keywords (§5.1 temporal, §5.2 require/observe syntax stable)
        "require" => Some(TokenKind::Require),
        "observe" => Some(TokenKind::Observe),
        "effects" => Some(TokenKind::Effects),
        "escapes-local-context" => Some(TokenKind::EscapesLocalContext),
        "local" => Some(TokenKind::LocalKw),
        "wallclock" => Some(TokenKind::WallclockKw),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_keywords() {
        assert_eq!(lookup("fn"), Some(TokenKind::Fn));
        assert_eq!(lookup("return"), Some(TokenKind::Return));
        for kw in ["let", "var", "if", "else", "true", "false"] {
            assert!(lookup(kw).is_some(), "`{kw}` should be a keyword");
        }
    }

    #[test]
    fn type_names_are_not_keywords() {
        assert_eq!(lookup("int32"), None);
    }

    #[test]
    fn module_keywords() {
        assert_eq!(lookup("use"), Some(TokenKind::Use));
        assert_eq!(lookup("public"), Some(TokenKind::Public));
    }
}
