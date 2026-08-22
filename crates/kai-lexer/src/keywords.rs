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
}
