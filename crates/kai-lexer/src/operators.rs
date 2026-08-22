use crate::cursor::Cursor;
use crate::token::TokenKind;

/// Maximal-munch operator matching.
///
/// Contract: the caller consumed `first` already; this function consumes any
/// continuation characters it matches and returns:
/// - `Some(Ok(kind))` — a complete operator
/// - `Some(Err(ch))` — `first` cannot start any operator (`&&`/`||` halves)
/// - `None` — `first` is not an operator starter (caller decides)
///
/// `-` never reaches here: the lexer must disambiguate `->` first.
pub fn scan(cursor: &mut Cursor, first: u8) -> Option<Result<TokenKind, char>> {
    let second = cursor.peek();

    let result = match first {
        b'=' => pair_or_single(cursor, second, b'=', TokenKind::EqEq, TokenKind::Eq),
        b'!' => pair_or_single(cursor, second, b'=', TokenKind::NotEq, TokenKind::Bang),
        b'<' => pair_or_single(cursor, second, b'=', TokenKind::Le, TokenKind::Lt),
        b'>' => pair_or_single(cursor, second, b'=', TokenKind::Ge, TokenKind::Gt),
        b'+' => pair_or_single(cursor, second, b'=', TokenKind::PlusEq, TokenKind::Plus),
        b'*' => pair_or_single(cursor, second, b'=', TokenKind::StarEq, TokenKind::Star),
        b'/' => pair_or_single(cursor, second, b'=', TokenKind::SlashEq, TokenKind::Slash),
        b'%' => single(TokenKind::Percent),
        // Lone '&' / '|' are errors; only '&&' / '||' exist.
        b'&' => require_double(cursor, second, b'&'),
        b'|' => require_double(cursor, second, b'|'),
        _ => return None,
    };

    Some(result)
}

fn pair_or_single(
    cursor: &mut Cursor,
    second: Option<u8>,
    pair: u8,
    both: TokenKind,
    one: TokenKind,
) -> Result<TokenKind, char> {
    if second == Some(pair) {
        cursor.bump();
        Ok(both)
    } else {
        Ok(one)
    }
}

fn single(one: TokenKind) -> Result<TokenKind, char> {
    Ok(one)
}

fn require_double(
    cursor: &mut Cursor,
    second: Option<u8>,
    expected: u8,
) -> Result<TokenKind, char> {
    if second == Some(expected) {
        cursor.bump();
        Ok(if expected == b'&' {
            TokenKind::AmpAmp
        } else {
            TokenKind::PipePipe
        })
    } else {
        Err(expected as char)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::Cursor;

    fn scan_all(src: &str) -> Vec<Result<TokenKind, char>> {
        let mut cursor = Cursor::new(src);
        let mut out = Vec::new();
        while let Some(first) = cursor.bump() {
            match scan(&mut cursor, first) {
                Some(result) => out.push(result),
                // Whitespace etc. is not an operator starter; keep scanning.
                None => continue,
            }
        }
        out
    }

    #[test]
    fn maximal_munch_prefers_pairs() {
        assert_eq!(
            scan_all("== ="),
            vec![Ok(TokenKind::EqEq), Ok(TokenKind::Eq)]
        );
        assert_eq!(
            scan_all("+ +="),
            vec![Ok(TokenKind::Plus), Ok(TokenKind::PlusEq)]
        );
        assert_eq!(scan_all("< <="), vec![Ok(TokenKind::Lt), Ok(TokenKind::Le)]);
    }

    #[test]
    fn logic_operators_require_doubles() {
        assert_eq!(
            scan_all("&& ||"),
            vec![Ok(TokenKind::AmpAmp), Ok(TokenKind::PipePipe)]
        );
        assert_eq!(scan_all("&"), vec![Err('&')]);
        assert_eq!(scan_all("|"), vec![Err('|')]);
    }

    #[test]
    fn non_operator_returns_none() {
        let mut cursor = Cursor::new("(");
        let first = cursor.bump().unwrap();
        assert!(scan(&mut cursor, first).is_none());
    }
}
