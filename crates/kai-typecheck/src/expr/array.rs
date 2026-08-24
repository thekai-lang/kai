#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]
use super::*;

pub(crate) fn array_lit(
    checker: &mut Checker,
    lit: &ArrayLitExpr,
    expected: Option<&KaiType>,
    lit_span: Span,
) -> TypedExpr {
    let elem_hint: Option<KaiType> = match expected {
        Some(KaiType::Array(elem)) => Some(elem.as_ref().clone()),
        _ => None,
    };

    if lit.elements.is_empty() {
        return match elem_hint {
            Some(elem) => TypedExpr::new(TypedExprKind::ArrayLit { elements: vec![] }, KaiType::Array(Box::new(elem))),
            None => {
                checker.error(error::empty_array_needs_annotation(lit_span));
                poisoned()
            }
        };
    }

    let mut typed: Vec<TypedExpr> = Vec::with_capacity(lit.elements.len());
    let mut elem_ty: Option<KaiType> = None;
    for element in &lit.elements {
        let value = lower(checker, element, elem_hint.clone());
        match &elem_ty {
            None => elem_ty = Some(value.ty.clone()),
            Some(expected_ty) => {
                if value.ty != *expected_ty {
                    let found = value.ty.clone();
                    checker.error(error::array_element_mismatch(expected_ty, found, element.span));
                }
            }
        }
        typed.push(value);
    }
    let elem = elem_ty.unwrap_or(KaiType::Int32);
    TypedExpr::new(
        TypedExprKind::ArrayLit { elements: typed },
        KaiType::Array(Box::new(elem)),
    )
}

/// `base[index]`: base must be `T[]`, index any integer width; the result is
/// a plain read of `T` (§9.3). Bounds are checked at RUNTIME in later
/// releases; here we only guarantee the shapes line up.

pub(crate) fn index_expr(checker: &mut Checker, indexed: &IndexExpr) -> TypedExpr {
    let base = lower(checker, &indexed.base, None);
    let Some((elem_ty, index)) =
        resolve_index_hop(checker, &base.ty, &indexed.index, indexed.rbracket)
    else {
        return poisoned();
    };
    TypedExpr::new(
        TypedExprKind::Index {
            base: Box::new(base),
            index: Box::new(index),
        },
        elem_ty,
    )
}

// -- v0.0.6 (§9.9a/§9.10) -------------------------------------------------------
