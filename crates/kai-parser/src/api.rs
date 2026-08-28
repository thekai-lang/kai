use crate::Parser;
use kai_ast::expr::{ApiContract, DslVariant};
use kai_ast::Ident;
use kai_lexer::TokenKind;

pub fn parse_api_variant(parser: &mut Parser, service: String, version: u32) -> DslVariant {
    let mut method = String::new();
    let mut path = String::new();
    let mut path_params = None;
    let mut query_params = None;
    let mut header_params = None;
    let mut body = None;
    let mut auth = None;

    if let TokenKind::Ident(m) = parser.peek().kind.clone() {
        method = m.clone();
        parser.bump(); // Consume METHOD
    } else {
        parser.diagnostics.push(crate::error::custom("expected HTTP method (e.g. GET, POST)", parser.span_here()));
    }

    // Accumulate tokens for the path until we hit a parameter block (`with`) or `}`
    while !parser.at_eof() {
        let peek = parser.peek().kind.clone();
        match &peek {
            TokenKind::Ident(name) if name.as_str() == "with" => {
                break; // Found block start
            }
            TokenKind::RBrace => {
                break; // End of DSL block
            }
            TokenKind::Slash => {
                path.push('/');
                parser.bump();
            }
            TokenKind::Minus => {
                path.push('-');
                parser.bump();
            }
            TokenKind::Dot => {
                path.push('.');
                parser.bump();
            }
            TokenKind::Ident(id) => {
                path.push_str(id);
                parser.bump();
            }
            TokenKind::LBrace => {
                path.push('{');
                parser.bump();
            }
            TokenKind::IntLit(n) => {
                path.push_str(&n.to_string());
                parser.bump();
            }
            TokenKind::FloatLit(f) => {
                path.push_str(&f.to_string());
                parser.bump();
            }
            TokenKind::StrLit(s) => {
                path.push_str(s);
                parser.bump();
            }
            _ => {
                break;
            }
        }
    }

    // Parse `with <category>:` blocks
    while !parser.at_eof() {
        if let TokenKind::Ident(name) = parser.peek().kind.clone() {
            if name.as_str() == "with" {
                parser.bump(); // consume `with`
                
                let category = parser.expect_ident("api parameter category (auth, path, query, header, body)");
                if parser.eat_simple(&TokenKind::Colon) {
                    if category.name.as_str() == "auth" {
                        auth = Some(Box::new(crate::expr::expr(parser)));
                    } else if matches!(category.name.as_str(), "body" | "path" | "query" | "header") {
                        if parser.eat_simple(&TokenKind::LBrace) {
                            let mut fields = Vec::new();
                            while !matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                                // Fields can be strings (e.g. header "X-Idempotency") or idents
                                let field_name = if let TokenKind::StrLit(s) = parser.peek().kind.clone() {
                                    let span = parser.span_here();
                                    parser.bump();
                                    Ident { name: s, span }
                                } else {
                                    parser.expect_ident("a field name")
                                };
                                
                                parser.expect_simple(&TokenKind::Colon);
                                let value = crate::expr::expr(parser);
                                fields.push((field_name, value));

                                if !parser.eat_simple(&TokenKind::Comma)
                                    && !matches!(parser.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                                        parser.diagnostics.push(crate::error::custom("expected `,` or `}` in block fields", parser.span_here()));
                                        break;
                                    }
                            }
                            parser.expect_simple(&TokenKind::RBrace); // `}`
                            
                            match category.name.as_str() {
                                "body" => body = Some(fields),
                                "path" => path_params = Some(fields),
                                "query" => query_params = Some(fields),
                                "header" => header_params = Some(fields),
                                _ => unreachable!(),
                            }
                        } else {
                            parser.diagnostics.push(crate::error::custom(format!("expected `{{` after `with {}:`", category.name), parser.span_here()));
                        }
                    } else {
                        parser.diagnostics.push(crate::error::custom(format!("unknown API parameter category `{}`", category.name), category.span));
                    }
                } else {
                    parser.diagnostics.push(crate::error::custom(format!("expected `:` after `{}`", category.name), parser.span_here()));
                }
            } else {
                break; // Hit something else (like RBrace)
            }
        } else {
            break; // Not an ident
        }
    }

    DslVariant::StructuredApi(ApiContract {
        service,
        version,
        method,
        path,
        path_params,
        query_params,
        header_params,
        body,
        auth,
    })
}
