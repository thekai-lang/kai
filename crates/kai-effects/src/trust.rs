//! Trust⟨C⟩ lowering for `require`/`observe` (whitepaper v0.20 §5.2.1–§5.2.2):
//! the single consumer-side definition of what each construct records.
//!
//! Scoping (v0.20): neither construct uses the call-graph inference
//! subsystem (`EffectName`/`EffectSet`, SCC/fixpoint, `effects { ... }`) —
//! but BOTH still lower through here, locally and immediately, so `kai debt`
//! (§5.6) and `@override` (§5.5) operate on `Trust<C>` uniformly (§8).
//!
//! Condition text is the raw source-text span (v0.22) — verbatim, including
//! embedded newlines; the JSON serializer escapes them so the one-record-
//! per-line format never breaks.

/// Canonical RFC 3339 / UTC / microsecond precision / mandatory `Z`
/// (§5.1.5's wire format, reused for record timestamps).
pub fn rfc3339_utc(micros_since_epoch: i64) -> String {
    let secs = micros_since_epoch.div_euclid(1_000_000);
    let micro = micros_since_epoch.rem_euclid(1_000_000);

    // Howard Hinnant's civil-from-days algorithm.
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{micro:06}Z",
            secs.rem_euclid(86_400) / 3600,
            secs.rem_euclid(3600) / 60,
            secs.rem_euclid(60))
}

/// Minimal JSON string escaping — enough for arbitrary source text:
/// quote, backslash, and the control range (incl. `\n`, `\r`, `\t`).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// One `observe` record (§5.2.2): Signal telemetry, never debt.
pub fn observe_jsonl(
    timestamp_micros: i64,
    location: &str,
    condition: &str,
    outcome: bool,
) -> String {
    format!(
        "{{\"timestamp\":\"{}\",\"location\":\"{}\",\"condition\":\"{}\",\"outcome\":{}}}",
        rfc3339_utc(timestamp_micros),
        json_escape(location),
        json_escape(condition),
        outcome
    )
}

/// One pre-ledger `require` violation record (§10.3, v0.21): appended
/// BEFORE the panic proceeds. `kind` matches §5.6's existing category so
/// v0.0.12 aggregation reads this file directly, no translation.
pub fn debt_correctness_jsonl(
    timestamp_micros: i64,
    location: &str,
    condition: &str,
) -> String {
    format!(
        "{{\"timestamp\":\"{}\",\"kind\":\"correctness\",\"location\":\"{}\",\"condition\":\"{}\",\"outcome\":false}}",
        rfc3339_utc(timestamp_micros),
        json_escape(location),
        json_escape(condition),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_instant() {
        // 2026-08-24T19:42:31.123456Z == 1787600551123456 µs
        assert_eq!(rfc3339_utc(1_787_600_551_123_456), "2026-08-24T19:42:31.123456Z");
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00.000000Z");
    }

    #[test]
    fn jsonl_escapes_newlines_and_quotes() {
        let line = observe_jsonl(0, "src/a.kai:1:1", "x > 0 && y != \"bad\"\nz", true);
        assert!(line.starts_with('{') && line.ends_with('}'));
        assert_eq!(line.matches('\n').count(), 0, "embedded newline must be escaped");
        assert!(line.contains("\\n"), "escaped newline present");
        assert!(line.contains("\"outcome\":true"));
    }

    #[test]
    fn debt_line_marks_correctness_kind() {
        let line = debt_correctness_jsonl(0, "src/b.kai:3:5", "user.age > 0");
        assert!(line.contains("\"kind\":\"correctness\""));
        assert!(line.contains("\"outcome\":false"));
    }
}
