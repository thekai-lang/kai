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
