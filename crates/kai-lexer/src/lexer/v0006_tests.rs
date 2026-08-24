use super::*;

fn kinds(src: &str) -> Vec<TokenKind> {
    lex(src).tokens.into_iter().map(|t| t.kind).collect()
}

#[test]
fn bare_underscore_is_its_own_token() {
    assert_eq!(kinds("_ = 1;"), vec![
        TokenKind::Underscore, TokenKind::Eq, TokenKind::IntLit(1), TokenKind::Semi,
        TokenKind::Eof
    ]);
}

#[test]
fn prefixed_underscore_stays_an_ident() {
    // §9.9b: only a BARE `_` is carved out of Ident.
    assert_eq!(
        kinds("_foo my_var __x"),
        vec![
            TokenKind::Ident("_foo".into()),
            TokenKind::Ident("my_var".into()),
            TokenKind::Ident("__x".into()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn some_none_catch_are_keywords() {
    assert!(matches!(kinds("Some")[0], TokenKind::SomeKw));
    assert!(matches!(kinds("None")[0], TokenKind::NoneKw));
    assert!(matches!(kinds("catch")[0], TokenKind::Catch));
    // Type names stay identifiers resolved by the type checker.
    assert_eq!(
        kinds("Result Optional"),
        vec![
            TokenKind::Ident("Result".into()),
            TokenKind::Ident("Optional".into()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn question_tokens_maximal_munch() {
    assert_eq!(
        kinds("a ?? b"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::QuestionQuestion,
            TokenKind::Ident("b".into()),
            TokenKind::Eof
        ]
    );
    assert_eq!(
        kinds("string?"),
        vec![TokenKind::Ident("string".into()), TokenKind::Question, TokenKind::Eof]
    );
}

#[test]
fn lone_pipe_lexes_but_double_wins() {
    assert_eq!(
        kinds("| ||"),
        vec![TokenKind::Pipe, TokenKind::PipePipe, TokenKind::Eof]
    );
}

#[test]
fn nul_escape_decodes() {
    // `\0` joined the locked escape set (whitepaper v0.12).
    match &lex(r#""a\0b""#).tokens[0].kind {
        TokenKind::StrLit(text) => assert_eq!(text.as_bytes(), b"a\0b"),
        other => panic!("expected string token, got {other:?}"),
    }
}
