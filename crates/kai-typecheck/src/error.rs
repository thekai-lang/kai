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
        format!("cannot assign to `{name}`: the binding is immutable"),
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

pub fn unsupported_expression(span: Span) -> Diagnostic {
    Diagnostic::error(
        "calls, field access, and struct literals are not supported yet",
        span,
    )
}

// -- v0.0.3 ------------------------------------------------------------------

pub fn unknown_function(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown function `{name}`"), span)
}

pub fn indirect_call(span: Span) -> Diagnostic {
    Diagnostic::error(
        "only direct calls to declared functions are supported",
        span,
    )
}

pub fn arg_count_mismatch(expected: usize, found: usize, span: Span) -> Diagnostic {
    let plural = if expected == 1 { "" } else { "s" };
    Diagnostic::error(
        format!("this function takes {expected} argument{plural}, but {found} were supplied"),
        span,
    )
}

pub fn arg_type_mismatch(
    param: KaiType,
    found: KaiType,
    position: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        format!("argument {position}: expected `{param}`, found `{found}`"),
        span,
    )
}

pub fn field_type_mismatch(
    field: &str,
    expected: KaiType,
    found: KaiType,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        format!("field `{field}`: expected `{expected}`, found `{found}`"),
        span,
    )
}

pub fn field_access_on_non_struct(ty: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("cannot access a field on a value of type `{ty}`"),
        span,
    )
}

pub fn no_such_field(ty_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("type `{ty_name}` has no field `{field}`"), span)
}

pub fn duplicate_field_init(field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("field `{field}` specified more than once"), span)
}

pub fn missing_field_in_lit(field: &str, ty_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("missing field `{field}` in `{ty_name}` literal"),
        span,
    )
}
