//! Human-facing diagnostic rendering:
//!
//! ```text
//! error: expected `;`, found `}`
//!  --> main.kai:1:26
//!  1 | fn main() -> int32 { return 0 }
//!    |                          ^
//! ```

use kai_diagnostics::{Diagnostic, Severity, SourceMap};

pub fn render(diagnostics: &[Diagnostic], source_name: &str, source: &str) -> String {
    let map = SourceMap::new(source);
    let mut out = String::new();

    for diag in diagnostics {
        let lc = map.line_col(diag.span.start);
        let line_text = map.line_text(diag.span.start);

        out.push_str(&format!("{}: {}\n", label(&diag.severity), diag.message));
        out.push_str(&format!(" --> {source_name}:{}:{}\n", lc.line, lc.col));
        out.push_str(&format!("{:>4} |\n", ""));
        out.push_str(&format!("{:>4} | {line_text}\n", lc.line));

        let caret_len = diag.span.end.saturating_sub(diag.span.start).max(1);
        let pad: String = " ".repeat(lc.col - 1);
        let carets: String = "^".repeat(caret_len);
        out.push_str(&format!("     | {pad}{carets}\n"));
    }

    out
}

fn label(severity: &Severity) -> String {
    match severity {
        Severity::Error => "error".to_string(),
        Severity::Warning => "warning".to_string(),
        Severity::Note => "note".to_string(),
    }
}
