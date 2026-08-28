
#[derive(Debug, Clone, PartialEq)]
pub enum SqlType {
    Uuid,
    String,
    Int32,
    Int64,
    Float64,
    Bool,
    Nullable(Box<SqlType>),
    Unknown(String),
}

impl SqlType {
    pub fn from_str(s: &str) -> Self {
        let is_nullable = s.ends_with('?');
        let base = s.trim_end_matches('?').trim();
        let ty = match base {
            "uuid" => SqlType::Uuid,
            "string" | "varchar" | "text" => SqlType::String,
            "int32" | "integer" => SqlType::Int32,
            "int64" | "bigint" => SqlType::Int64,
            "float64" | "double precision" | "float" => SqlType::Float64,
            "bool" | "boolean" => SqlType::Bool,
            _ => SqlType::Unknown(base.to_string()),
        };
        if is_nullable {
            SqlType::Nullable(Box::new(ty))
        } else {
            ty
        }
    }
}
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct SqlSnapshot {
    pub version: u32,
    pub source_kind: Option<String>,
    pub source_database: Option<String>,
    pub captured_at: Option<String>,
    pub tables: HashMap<String, SqlTable>, // renamed from schema for internal consistency, but parses from "schema"
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlTable {
    pub columns: HashMap<String, SqlType>, // column_name -> sql_type
}

/// A highly focused, zero-dependency JSON parser strictly for `SqlSnapshot`.
pub fn parse_snapshot(json: &str) -> Result<SqlSnapshot, String> {
    let tokens = tokenize(json)?;
    let mut pos = 0;

    let mut version = None;
    let mut source_kind = None;
    let mut source_database = None;
    let mut captured_at = None;
    let mut tables = HashMap::new();

    expect(&tokens, &mut pos, Token::LBrace)?;

    while pos < tokens.len() && tokens[pos] != Token::RBrace {
        let key = expect_string(&tokens, &mut pos)?;
        expect(&tokens, &mut pos, Token::Colon)?;

        match key.as_str() {
            "version" => {
                let v = expect_number(&tokens, &mut pos)?;
                version = Some(v as u32);
            }
            "source" => {
                expect(&tokens, &mut pos, Token::LBrace)?;
                while pos < tokens.len() && tokens[pos] != Token::RBrace {
                    let k = expect_string(&tokens, &mut pos)?;
                    expect(&tokens, &mut pos, Token::Colon)?;
                    match k.as_str() {
                        "kind" => source_kind = Some(expect_string(&tokens, &mut pos)?),
                        "database" => source_database = Some(expect_string(&tokens, &mut pos)?),
                        _ => skip_value(&tokens, &mut pos)?,
                    }
                    if pos < tokens.len() && tokens[pos] == Token::Comma { pos += 1; }
                }
                expect(&tokens, &mut pos, Token::RBrace)?;
            }
            "captured_at" => {
                captured_at = Some(expect_string(&tokens, &mut pos)?);
            }
            "schema" | "tables" => {
                tables = parse_tables(&tokens, &mut pos)?;
            }
            _ => {
                // Ignore unknown top-level keys
                skip_value(&tokens, &mut pos)?;
            }
        }

        if pos < tokens.len() && tokens[pos] == Token::Comma {
            pos += 1;
        }
    }

    expect(&tokens, &mut pos, Token::RBrace)?;

    let version = version.ok_or_else(|| "missing 'version' field".to_string())?;

    Ok(SqlSnapshot { version, source_kind, source_database, captured_at, tables })
}

fn parse_tables(tokens: &[Token], pos: &mut usize) -> Result<HashMap<String, SqlTable>, String> {
    let mut tables = HashMap::new();
    expect(tokens, pos, Token::LBrace)?;

    while *pos < tokens.len() && tokens[*pos] != Token::RBrace {
        let table_name = expect_string(tokens, pos)?;
        expect(tokens, pos, Token::Colon)?;

        let mut columns = HashMap::new();
        expect(tokens, pos, Token::LBrace)?;

        while *pos < tokens.len() && tokens[*pos] != Token::RBrace {
            let key = expect_string(tokens, pos)?;
            expect(tokens, pos, Token::Colon)?;

            // In the new schema format, it's just "id": "uuid"
            // In the old format, it might be "columns": { "id": "uuid" }
            if key == "columns" {
                if tokens[*pos] == Token::LBrace {
                    expect(tokens, pos, Token::LBrace)?;
                    while *pos < tokens.len() && tokens[*pos] != Token::RBrace {
                        let col_name = expect_string(tokens, pos)?;
                        expect(tokens, pos, Token::Colon)?;
                        let col_type = expect_string(tokens, pos)?;
                        columns.insert(col_name, SqlType::from_str(&col_type));

                        if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                            *pos += 1;
                        }
                    }
                    expect(tokens, pos, Token::RBrace)?;
                } else {
                    skip_value(tokens, pos)?;
                }
            } else if let Token::String(_) = tokens[*pos] {
                // Direct column mapping "id": "uuid"
                let col_type = expect_string(tokens, pos)?;
                columns.insert(key, SqlType::from_str(&col_type));
            } else {
                skip_value(tokens, pos)?;
            }

            if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                *pos += 1;
            }
        }
        expect(tokens, pos, Token::RBrace)?;

        tables.insert(table_name, SqlTable { columns });

        if *pos < tokens.len() && tokens[*pos] == Token::Comma {
            *pos += 1;
        }
    }

    expect(tokens, pos, Token::RBrace)?;
    Ok(tables)
}

fn skip_value(tokens: &[Token], pos: &mut usize) -> Result<(), String> {
    if *pos >= tokens.len() {
        return Err("unexpected EOF".into());
    }
    match tokens[*pos] {
        Token::LBrace => {
            *pos += 1;
            while *pos < tokens.len() && tokens[*pos] != Token::RBrace {
                // key
                if let Token::String(_) = tokens[*pos] {
                    *pos += 1;
                } else {
                    return Err("expected string key".into());
                }
                expect(tokens, pos, Token::Colon)?;
                skip_value(tokens, pos)?;
                if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                    *pos += 1;
                }
            }
            expect(tokens, pos, Token::RBrace)?;
        }
        Token::LBracket => {
            *pos += 1;
            while *pos < tokens.len() && tokens[*pos] != Token::RBracket {
                skip_value(tokens, pos)?;
                if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                    *pos += 1;
                }
            }
            expect(tokens, pos, Token::RBracket)?;
        }
        _ => {
            // string, number, true, false, null
            *pos += 1;
        }
    }
    Ok(())
}

fn expect(tokens: &[Token], pos: &mut usize, expected: Token) -> Result<(), String> {
    if *pos >= tokens.len() {
        return Err(format!("expected {:?}, found EOF", expected));
    }
    if tokens[*pos] != expected {
        return Err(format!("expected {:?}, found {:?}", expected, tokens[*pos]));
    }
    *pos += 1;
    Ok(())
}

fn expect_string(tokens: &[Token], pos: &mut usize) -> Result<String, String> {
    if *pos >= tokens.len() {
        return Err("expected string, found EOF".into());
    }
    if let Token::String(s) = &tokens[*pos] {
        let val = s.clone();
        *pos += 1;
        Ok(val)
    } else {
        Err(format!("expected string, found {:?}", tokens[*pos]))
    }
}

fn expect_number(tokens: &[Token], pos: &mut usize) -> Result<i64, String> {
    if *pos >= tokens.len() {
        return Err("expected number, found EOF".into());
    }
    if let Token::Number(n) = tokens[*pos] {
        *pos += 1;
        Ok(n)
    } else {
        Err(format!("expected number, found {:?}", tokens[*pos]))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    String(String),
    Number(i64),
    // True, False, Null omitted for simplicity as they aren't in our subset
}

fn tokenize(s: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '{' => { tokens.push(Token::LBrace); i += 1; }
            '}' => { tokens.push(Token::RBrace); i += 1; }
            '[' => { tokens.push(Token::LBracket); i += 1; }
            ']' => { tokens.push(Token::RBracket); i += 1; }
            ':' => { tokens.push(Token::Colon); i += 1; }
            ',' => { tokens.push(Token::Comma); i += 1; }
            '"' => {
                let mut val = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1; // skip slash
                        // simplistic unescape for now
                        val.push(chars[i]);
                    } else {
                        val.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("unterminated string literal".into());
                }
                i += 1; // skip closing quote
                tokens.push(Token::String(val));
            }
            c if c.is_ascii_digit() || c == '-' => {
                let mut num_str = String::new();
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                    num_str.push(chars[i]);
                    i += 1;
                }
                let n = num_str.parse::<i64>().map_err(|_| format!("invalid number: {}", num_str))?;
                tokens.push(Token::Number(n));
            }
            c => return Err(format!("unexpected character: {}", c)),
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_snapshot() {
        let json = r#"{
            "version": 12,
            "tables": {
                "users": {
                    "columns": {
                        "id": "uuid",
                        "name": "string"
                    }
                }
            }
        }"#;

        let snap = parse_snapshot(json).unwrap();
        assert_eq!(snap.version, 12);
        assert_eq!(snap.tables.len(), 1);
        let users = snap.tables.get("users").unwrap();
        assert_eq!(users.columns.get("id").unwrap(), &SqlType::Uuid);
        assert_eq!(users.columns.get("name").unwrap(), &SqlType::String);
    }
}
