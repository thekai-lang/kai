//! Byte-offset spans and their mapping back to human line/column positions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(a: Span, b: Span) -> Self {
        Self {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

/// 1-based source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

/// Maps byte offsets in one source text to line/column positions.
#[derive(Debug, Clone, Copy)]
pub struct SourceMap<'src> {
    src: &'src str,
}

impl<'src> SourceMap<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src }
    }

    /// Byte offset -> 1-based (line, column). Offsets past the end clamp to the
    /// last position.
    pub fn line_col(&self, offset: usize) -> LineCol {
        let offset = offset.min(self.src.len());
        let mut line = 1;
        let mut line_start = 0;

        for (idx, byte) in self.src.bytes().enumerate() {
            if idx >= offset {
                break;
            }
            if byte == b'\n' {
                line += 1;
                line_start = idx + 1;
            }
        }

        LineCol {
            line,
            col: offset - line_start + 1,
        }
    }

    /// Full text of the 1-based line containing `offset`, without the trailing
    /// newline.
    pub fn line_text(&self, offset: usize) -> &'src str {
        let offset = offset.min(self.src.len());
        let start = self.src[..offset].rfind('\n').map_or(0, |pos| pos + 1);
        let end = self.src[offset..]
            .find('\n')
            .map_or(self.src.len(), |rel| offset + rel);
        self.src[start..end].trim_end_matches('\r')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_merge_extends_bounds() {
        assert_eq!(
            Span::merge(Span::new(2, 5), Span::new(0, 3)),
            Span::new(0, 5)
        );
    }

    #[test]
    fn maps_offsets_to_line_col() {
        let src = "fn main() {\n    return 0;\n}\n";
        let map = SourceMap::new(src);

        assert_eq!(map.line_col(0), LineCol { line: 1, col: 1 });
        let ret_off = src.find("return").unwrap();
        assert_eq!(map.line_col(ret_off), LineCol { line: 2, col: 5 });
    }

    #[test]
    fn extracts_line_text() {
        let src = "fn main() {\n    return 0;\n}";
        let map = SourceMap::new(src);
        assert_eq!(map.line_text(src.find("return").unwrap()), "    return 0;");
    }
}
