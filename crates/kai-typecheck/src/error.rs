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

pub fn arg_count_mismatch(name: &str, expected: usize, found: usize, span: Span) -> Diagnostic {
    let plural = if expected == 1 { "" } else { "s" };
    Diagnostic::error(
        format!("function `{name}` takes {expected} argument{plural}, but {found} were supplied"),
        span,
    )
}

pub fn arg_type_mismatch(
    name: &str,
    param: KaiType,
    found: KaiType,
    position: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        format!("argument {position} of `{name}`: expected `{param}`, found `{found}`"),
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

// -- v0.0.4: qualified (module) references -----------------------------------

pub fn unknown_module(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown module `{name}`"), span)
}

pub fn unknown_qualified_function(path: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown function `{path}`"), span)
}

pub fn private_function(path: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("function `{path}` is not public"), span)
}

pub fn unknown_qualified_type(path: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown type `{path}`"), span)
}

pub fn private_type(path: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("type `{path}` is not public"), span)
}

// -- v0.0.5 -------------------------------------------------------------------

pub(crate) fn index_on_non_array(ty: &KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("cannot index into a value of type `{ty}` (only arrays are indexable)"),
        span,
    )
}

pub(crate) fn index_not_integer(ty: &KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(format!("array index must be an integer, found `{ty}`"), span)
}

pub(crate) fn for_iterable_not_array(ty: &KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("for..in iterates arrays only, found `{ty}`"),
        span,
    )
}

pub(crate) fn empty_array_needs_annotation(span: Span) -> Diagnostic {
    Diagnostic::error("empty array literal requires a type annotation", span)
}

pub(crate) fn array_element_mismatch(expected: &KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("array elements must share one type: expected `{expected}`, found `{found}`"),
        span,
    )
}

pub(crate) fn assign_type_mismatch(place: &KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("cannot assign `{found}` to a place of type `{place}`"),
        span,
    )
}

// -- v0.0.6 (§9.9a/§9.10) -------------------------------------------------------

pub(crate) fn none_needs_annotation(span: Span) -> Diagnostic {
    Diagnostic::error("bare `None` requires a type annotation to fix its payload", span)
}

pub(crate) fn coalesce_on_non_optional(found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`??` needs an `Optional` on the left, found `{found}`"),
        span,
    )
}

pub(crate) fn coalesce_default_mismatch(expected: KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`??` fallback must be `{expected}`, found `{found}`"),
        span,
    )
}

pub(crate) fn unwrap_or_receiver(found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`unwrap_or` expects an `Optional` or `Result` receiver, found `{found}`"),
        span,
    )
}

pub(crate) fn unwrap_or_default_mismatch(expected: KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`unwrap_or` default must be `{expected}`, found `{found}`"),
        span,
    )
}

pub(crate) fn unwrap_or_arity(found: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`unwrap_or` takes exactly one argument, found {found}"),
        span,
    )
}

pub(crate) fn catch_on_non_result(found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("`catch` handles `Result` values only, found `{found}` (Optionals use `??`)"),
        span,
    )
}

pub(crate) fn catch_tail_mismatch(expected: KaiType, found: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("catch block must produce `{expected}`, found `{found}`"),
        span,
    )
}

pub(crate) fn compensate_on_non_call(span: Span) -> Diagnostic {
    Diagnostic::error(
        "`compensate` can only be attached to a function call (§5.3)",
        span,
    )
}

pub(crate) fn closure_needs_return(ret: KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("closure with return type `{ret}` must end in a value or return"),
        span,
    )
}

pub(crate) fn closure_capture_banned(name: &str, ty: &KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!(
            "cannot capture `{name}`: `{ty}` is or contains a closure — reference cycles are rejected (§9.10)"
        ),
        span,
    )
}

pub(crate) fn discard_tagged(ty: &KaiType, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!(
            "discarding a `{ty}` silently hides its state — write `_ = expr;` to discard explicitly"
        ),
        span,
    )
}

pub(crate) fn ok_needs_annotation(span: Span) -> Diagnostic {
    Diagnostic::error(
        "bare `Ok` requires a type annotation to fix its error payload (like `None` and `[]`)",
        span,
    )
}

pub(crate) fn err_needs_annotation(span: Span) -> Diagnostic {
    Diagnostic::error(
        "bare `Err` requires a type annotation to fix its ok payload (like `None` and `[]`)",
        span,
    )
}

pub(crate) fn temporal_zero_duration(span: Span) -> Diagnostic {
    Diagnostic::error("temporal duration must be non-zero (e.g. `30m`, not `0m`)", span)
}

#[allow(dead_code)]
pub(crate) fn effect_mismatch(declared: &str, inferred: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        format!("declared effects {declared} do not cover inferred effects {inferred} (inferred ⊆ declared must hold, §5.1.2)"),
        span,
    )
}
