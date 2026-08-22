use kai_diagnostics::{Diagnostic, Span};
use kai_tast::KaiType;

pub fn unknown_type(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown type `{name}`"), span)
}

pub fn literal_out_of_range(max_inclusive: u64, ty: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("integer literal does not fit in `{ty}` (max {max_inclusive})"),
        span,
    )
}

pub fn return_type_mismatch(expected: KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("return type mismatch: expected `{expected}`, found `{found}`"),
        span,
    )
}

pub fn missing_return_value(return_type: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`return` must produce a `{return_type}` value"),
        span,
    )
}

pub fn function_needs_return(name: &str, return_type: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("function `{name}` declared to return `{return_type}` but has no `return`"),
        span,
    )
}
