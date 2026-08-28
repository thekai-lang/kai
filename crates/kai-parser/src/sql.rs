use crate::error;
use crate::Parser;
use kai_ast::expr::{
    DslBlockExpr, DslVariant,
};
use kai_diagnostics::Span;
use kai_lexer::TokenKind;

pub(crate) fn parse_dsl_block(parser: &mut Parser, start_span: Span) -> kai_ast::expr::Expr {
    let mut span = start_span;
    parser.bump(); // consume 'dsl'

    // Expect 'sql'
    let kind = match parser.peek().kind.clone() {
        TokenKind::Ident(name) => {
            let token = parser.bump();
            span = Span::merge(span, token.span);
            name
        }
        _ => {
            let found = parser.peek().clone();
            parser
                .diagnostics
                .push(error::custom("expected `sql` or `api` after `dsl`", found.span));
            return kai_ast::expr::Expr {
                kind: kai_ast::expr::ExprKind::Invalid,
                span,
            };
        }
    };

    // Optional `raw`
    let mut is_raw = false;
    if let TokenKind::Ident(name) = parser.peek().kind.clone()
        && name == "raw" {
            is_raw = true;
            let token = parser.bump();
            span = Span::merge(span, token.span);
        }

    // Expect `(`
    if !parser.eat_simple(&TokenKind::LParen) {
        parser.diagnostics.push(error::custom(
            "expected `(` for snapshot info",
            parser.span_here(),
        ));
    }

    let mut service_name = String::new();
    if kind == "api" {
        if let TokenKind::StrLit(s) = parser.peek().kind.clone() {
            service_name = s;
            parser.bump(); // consume string
            if !parser.eat_simple(&TokenKind::Comma) {
                parser.diagnostics.push(error::custom("expected `,` after service name", parser.span_here()));
            }
        } else {
            parser.diagnostics.push(error::custom("expected service name string (e.g., \"stripe\")", parser.span_here()));
        }
    }

    let version = match parser.peek().kind.clone() {
        TokenKind::Ident(name) if name.starts_with('v') => {
            let _token = parser.bump();
            name[1..].parse::<u32>().unwrap_or(0)
        }
        _ => {
            parser.diagnostics.push(error::custom(
                "expected version identifier (e.g., `v12`)",
                parser.span_here(),
            ));
            0
        }
    };

    if !parser.eat_simple(&TokenKind::RParen) {
        parser.diagnostics.push(error::custom(
            "expected `)` after version",
            parser.span_here(),
        ));
    }

    // Optional return type `-> Type`
    let mut return_ty = None;
    if parser.eat_simple(&TokenKind::Arrow) {
        return_ty = Some(Box::new(crate::ty::ty(parser)));
    }

    // Block
    if !parser.eat_simple(&TokenKind::LBrace) {
        parser.diagnostics.push(error::custom(
            "expected `{` for dsl block",
            parser.span_here(),
        ));
        return kai_ast::expr::Expr {
            kind: kai_ast::expr::ExprKind::Invalid,
            span,
        };
    }

    let variant = if is_raw {
        parse_raw_variant(parser)
    } else if kind == "api" {
        crate::api::parse_api_variant(parser, service_name, version)
    } else {
        parse_structured_variant(parser)
    };

    let rbrace_span = parser.expect_simple(&TokenKind::RBrace);
    span = Span::merge(span, rbrace_span);

    kai_ast::expr::Expr {
        kind: kai_ast::expr::ExprKind::DslBlock(DslBlockExpr {
            kind,
            variant,
            version,
            return_ty,
            span,
        }),
        span,
    }
}

fn parse_raw_variant(parser: &mut Parser) -> DslVariant {
    let query_str = match parser.peek().kind.clone() {
        TokenKind::StrLit(s) => {
            parser.bump();
            s
        }
        _ => {
            parser.diagnostics.push(error::custom(
                "expected string literal in `dsl sql raw` block",
                parser.span_here(),
            ));
            String::new()
        }
    };
    DslVariant::Raw(query_str)
}

fn parse_structured_variant(parser: &mut Parser) -> DslVariant {
    let mut select = Vec::new();
    let mut from = kai_ast::expr::SqlTableRef { name: String::new(), span: kai_diagnostics::Span::new(0, 0) };

    // Parse SELECT
    if let TokenKind::Ident(name) = parser.peek().kind.clone() {
        if name.eq_ignore_ascii_case("select") {
            parser.bump(); // consume 'select'
            
            // parse comma-separated columns
            loop {
                // Peek next token. If it's 'from', break.
                if let TokenKind::Ident(ident) = parser.peek().kind.clone() {
                    if ident.eq_ignore_ascii_case("from") {
                        break;
                    }
                    
                    let token = parser.bump();
                    let mut col_name = ident.clone();
                    let mut qualifier = None;
                    let mut span = token.span;
                    
                    // Check for dot (qualification)
                    if parser.eat_simple(&TokenKind::Dot) {
                        qualifier = Some(col_name.clone());
                        if let TokenKind::Ident(field) = parser.peek().kind.clone() {
                            let f_tok = parser.bump();
                            col_name = field;
                            span = kai_diagnostics::Span::merge(span, f_tok.span);
                        } else {
                            parser.diagnostics.push(crate::error::custom("expected column name after `.`", parser.span_here()));
                        }
                    }
                    
                    let mut alias = None;
                    if let TokenKind::Ident(as_kw) = parser.peek().kind.clone()
                        && as_kw.eq_ignore_ascii_case("as") {
                            parser.bump(); // consume 'as'
                            if let TokenKind::Ident(alias_name) = parser.peek().kind.clone() {
                                parser.bump();
                                alias = Some(alias_name);
                            } else {
                                parser.diagnostics.push(crate::error::custom("expected alias name after `as`", parser.span_here()));
                            }
                        }
                    
                    select.push(kai_ast::expr::SqlSelectExpr {
                        expr: kai_ast::expr::SqlExpr::Column {
                            qualifier,
                            name: col_name,
                            span,
                        },
                        alias,
                    });
                    
                    if parser.eat_simple(&TokenKind::Comma) {
                        continue;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        } else {
            parser.diagnostics.push(crate::error::custom("expected `select`", parser.span_here()));
        }
    } else {
        parser.diagnostics.push(crate::error::custom("expected `select`", parser.span_here()));
    }

    // Parse FROM
    if let TokenKind::Ident(name) = parser.peek().kind.clone() {
        if name.eq_ignore_ascii_case("from") {
            parser.bump(); // consume 'from'
            
            if let TokenKind::Ident(table_name) = parser.peek().kind.clone() {
                let token = parser.bump();
                from = kai_ast::expr::SqlTableRef {
                    name: table_name,
                    span: token.span,
                };
            } else {
                parser.diagnostics.push(crate::error::custom("expected table name after `from`", parser.span_here()));
            }
        }
    } else {
        parser.diagnostics.push(crate::error::custom("expected `from`", parser.span_here()));
    }

    let mut joins = Vec::new();
    while let TokenKind::Ident(name) = parser.peek().kind.clone() {
        if name.eq_ignore_ascii_case("join") {
            parser.bump(); // consume 'join'
            
            let mut join_table = kai_ast::expr::SqlTableRef { name: String::new(), span: kai_diagnostics::Span::new(0, 0) };
            if let TokenKind::Ident(table_name) = parser.peek().kind.clone() {
                let token = parser.bump();
                join_table = kai_ast::expr::SqlTableRef {
                    name: table_name,
                    span: token.span,
                };
            } else {
                parser.diagnostics.push(crate::error::custom("expected table name after `join`", parser.span_here()));
            }
            
            // expect 'on'
            if let TokenKind::Ident(on_kw) = parser.peek().kind.clone() {
                if on_kw.eq_ignore_ascii_case("on") {
                    parser.bump(); // consume 'on'
                } else {
                    parser.diagnostics.push(crate::error::custom("expected `on` after join table", parser.span_here()));
                }
            } else {
                parser.diagnostics.push(crate::error::custom("expected `on` after join table", parser.span_here()));
            }
            
            let on_clause = parse_sql_expr(parser);
            joins.push(kai_ast::expr::SqlJoin {
                table: join_table,
                on_clause,
            });
            
        } else {
            break;
        }
    }

    // Optional WHERE
    let mut where_clause = None;
    if let TokenKind::Ident(name) = parser.peek().kind.clone()
        && name.eq_ignore_ascii_case("where") {
            parser.bump(); // consume 'where'
            where_clause = Some(parse_sql_expr(parser));
        }

    // Parse ORDER BY
    let mut order_by = Vec::new();
    if let TokenKind::Ident(name) = parser.peek().kind.clone()
        && name.eq_ignore_ascii_case("order") {
            parser.bump(); // consume 'order'
            
            if let TokenKind::Ident(by_kw) = parser.peek().kind.clone() {
                if by_kw.eq_ignore_ascii_case("by") {
                    parser.bump(); // consume 'by'
                } else {
                    parser.diagnostics.push(crate::error::custom("expected `by` after `order`", parser.span_here()));
                }
            } else {
                parser.diagnostics.push(crate::error::custom("expected `by` after `order`", parser.span_here()));
            }
            
            loop {
                let expr = parse_sql_expr(parser);
                let mut descending = false;
                
                if let TokenKind::Ident(dir) = parser.peek().kind.clone() {
                    if dir.eq_ignore_ascii_case("desc") {
                        descending = true;
                        parser.bump();
                    } else if dir.eq_ignore_ascii_case("asc") {
                        parser.bump();
                    }
                }
                
                order_by.push(kai_ast::expr::SqlOrderBy { expr, descending });
                
                if parser.eat_simple(&TokenKind::Comma) {
                    continue;
                } else {
                    break;
                }
            }
        }

    // Parse LIMIT
    let mut limit = None;
    if let TokenKind::Ident(name) = parser.peek().kind.clone()
        && name.eq_ignore_ascii_case("limit") {
            parser.bump(); // consume 'limit'
            
            if let TokenKind::IntLit(val) = parser.peek().kind.clone() {
                parser.bump();
                limit = Some(val);
            } else {
                parser.diagnostics.push(crate::error::custom("expected integer literal after `limit`", parser.span_here()));
            }
        }

    // Consume until RBrace (for anything unsupported)
    while !parser.at_eof() && parser.peek().kind != TokenKind::RBrace {
        parser.bump();
    }

    DslVariant::StructuredSql(kai_ast::expr::SqlQuery {
        select,
        from,
        joins,
        where_clause,
        group_by: Vec::new(),
        order_by,
        limit,
    })
}

fn parse_sql_expr(parser: &mut Parser) -> kai_ast::expr::SqlExpr {
    parse_sql_or(parser)
}

fn parse_sql_or(parser: &mut Parser) -> kai_ast::expr::SqlExpr {
    let mut expr = parse_sql_and(parser);
    while let TokenKind::Ident(op) = parser.peek().kind.clone() {
        if op.eq_ignore_ascii_case("or") {
            parser.bump();
            let rhs = parse_sql_and(parser);
            expr = kai_ast::expr::SqlExpr::BinaryOp(Box::new(expr), kai_ast::expr::SqlOp::Or, Box::new(rhs));
        } else {
            break;
        }
    }
    expr
}

fn parse_sql_and(parser: &mut Parser) -> kai_ast::expr::SqlExpr {
    let mut expr = parse_sql_cmp(parser);
    while let TokenKind::Ident(op) = parser.peek().kind.clone() {
        if op.eq_ignore_ascii_case("and") {
            parser.bump();
            let rhs = parse_sql_cmp(parser);
            expr = kai_ast::expr::SqlExpr::BinaryOp(Box::new(expr), kai_ast::expr::SqlOp::And, Box::new(rhs));
        } else {
            break;
        }
    }
    expr
}

fn parse_sql_cmp(parser: &mut Parser) -> kai_ast::expr::SqlExpr {
    let mut expr = parse_sql_primary(parser);
    
    let op = match parser.peek().kind {
        TokenKind::Eq | TokenKind::EqEq => Some(kai_ast::expr::SqlOp::Eq),
        TokenKind::NotEq => Some(kai_ast::expr::SqlOp::NotEq),
        TokenKind::Lt => Some(kai_ast::expr::SqlOp::Lt),
        TokenKind::Gt => Some(kai_ast::expr::SqlOp::Gt),
        TokenKind::Le => Some(kai_ast::expr::SqlOp::Le),
        TokenKind::Ge => Some(kai_ast::expr::SqlOp::Ge),
        _ => None,
    };
    
    if let Some(sql_op) = op {
        parser.bump();
        let rhs = parse_sql_primary(parser);
        expr = kai_ast::expr::SqlExpr::BinaryOp(Box::new(expr), sql_op, Box::new(rhs));
    }
    
    expr
}

fn parse_sql_primary(parser: &mut Parser) -> kai_ast::expr::SqlExpr {
    match parser.peek().kind.clone() {
        TokenKind::IntLit(val) => {
            let token = parser.bump();
            kai_ast::expr::SqlExpr::IntLit { value: val as i64, span: token.span }
        }
        TokenKind::StrLit(val) => {
            let token = parser.bump();
            kai_ast::expr::SqlExpr::StringLit { value: val.clone(), span: token.span }
        }
        TokenKind::True => {
            let token = parser.bump();
            kai_ast::expr::SqlExpr::BoolLit { value: true, span: token.span }
        }
        TokenKind::False => {
            let token = parser.bump();
            kai_ast::expr::SqlExpr::BoolLit { value: false, span: token.span }
        }
        TokenKind::Ident(ident) => {
            let token = parser.bump();
            let mut col_name = ident.clone();
            let mut qualifier = None;
            let mut span = token.span;
            
            if parser.eat_simple(&TokenKind::Dot) {
                qualifier = Some(col_name.clone());
                if let TokenKind::Ident(field) = parser.peek().kind.clone() {
                    let f_tok = parser.bump();
                    col_name = field;
                    span = kai_diagnostics::Span::merge(span, f_tok.span);
                } else {
                    parser.diagnostics.push(crate::error::custom("expected column name after `.`", parser.span_here()));
                }
            }
            kai_ast::expr::SqlExpr::Column { qualifier, name: col_name, span }
        }
        _ => {
            parser.diagnostics.push(crate::error::custom("expected SQL expression", parser.span_here()));
            let token = parser.bump(); // Prevent infinite loops
            kai_ast::expr::SqlExpr::Column { qualifier: None, name: String::new(), span: token.span }
        }
    }
}

