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
