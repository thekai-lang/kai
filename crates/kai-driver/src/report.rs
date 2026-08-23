//! Human-facing diagnostic rendering:
//!
//! ```text
//! error: expected `;`, found `}`
//!  --> main.kai:1:26
//!  1 | fn main() -> int32 { return 0 }
//!    |                          ^
//! ```
//!
//! Multi-file programs render each diagnostic against its OWN file's source
//! (`Diagnostic::file` picks the snippet; the first source is the fallback).

use std::collections::HashMap;

use kai_diagnostics::{Diagnostic, Severity, SourceMap};

pub fn render(diagnostics: &[Diagnostic], source_name: &str, source: &str) -> String {
    let sources = vec![(source_name.to_string(), source.to_string())];
    render_multi(diagnostics, &sources)
}

pub fn render_multi(diagnostics: &[Diagnostic], sources: &[(String, String)]) -> String {
    // One SourceMap per distinct file, built lazily.
    let mut maps: HashMap<String, SourceMap> = HashMap::new();
    let mut out = String::new();

    for diag in diagnostics {
        let Some((name, src)) = pick_source(diag, sources) else {
            // Diagnostic names a file we never saw (or no sources at all):
            // message-only output beats fabricating a wrong snippet.
            out.push_str(&format!("{}: {}\n", label(&diag.severity), diag.message));
            continue;
        };
        let map = maps.entry(name.to_string()).or_insert_with(|| SourceMap::new(src));

        out.push_str(&format!("{}: {}\n", label(&diag.severity), diag.message));
        let lc = map.line_col(diag.span.start);
        let line_text = map.line_text(diag.span.start);

        out.push_str(&format!(" --> {name}:{}:{}\n", lc.line, lc.col));
        out.push_str(&format!("{:>4} |\n", ""));
        out.push_str(&format!("{:>4} | {line_text}\n", lc.line));

        let caret_len = diag.span.end.saturating_sub(diag.span.start).max(1);
        let pad: String = " ".repeat(lc.col - 1);
        let carets: String = "^".repeat(caret_len);
        out.push_str(&format!("     | {pad}{carets}\n"));
    }

    out
}

/// The diagnostic's own file when we have it; otherwise the first source.
/// `None` when no snippet can be rendered at all.
fn pick_source<'a>(
    diag: &Diagnostic,
    sources: &'a [(String, String)],
) -> Option<(&'a str, &'a str)> {
    match diag.file.as_deref() {
        Some(file) => sources.iter().find_map(|(name, src)| {
            (name == file).then_some((name.as_str(), src.as_str()))
        }),
        None if !sources.is_empty() => {
            let (name, src) = &sources[0];
            Some((name.as_str(), src.as_str()))
        }
        None => None,
    }
}

fn label(severity: &Severity) -> String {
    match severity {
        Severity::Error => "error".to_string(),
        Severity::Warning => "warning".to_string(),
        Severity::Note => "note".to_string(),
    }
}
