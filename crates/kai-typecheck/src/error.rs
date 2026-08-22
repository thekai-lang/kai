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

pub fn undeclared_variable(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("undeclared variable `{name}`"), span)
}

pub fn assign_to_immutable(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("cannot assign to `{name}`: declared with `let` (use `var` for mutable bindings)"),
        span,
    )
}

pub fn duplicate_local(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("variable `{name}` is already declared in this scope"),
        span,
    )
}

pub fn init_type_mismatch(name: &str, expected: KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("cannot initialize `{name}`: expected `{expected}`, found `{found}`"),
        span,
    )
}

pub fn unit_binding(span: Span) -> Diagnostic {
    Diagnostic::error("bindings of type `unit` are not supported yet", span)
}

pub fn condition_not_bool(found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(format!("condition must be `bool`, found `{found}`"), span)
}

pub fn operand_type_mismatch(op: &str, ty: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("operator `{op}` cannot be applied to type `{ty}`"),
        span,
    )
}

pub fn binary_type_mismatch(op: &str, lhs: KaiType, rhs: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!(
            "type mismatch in binary expression: `{op}` requires equal types, \
             found `{lhs}` and `{rhs}`"
        ),
        span,
    )
}

pub fn mod_requires_integers(span: Span) -> Diagnostic {
    Diagnostic::error("`%` requires integer operands", span)
}

pub fn invalid_expression(span: Span) -> Diagnostic {
    Diagnostic::error("expression could not be parsed", span)
}
