pub mod snapshot;

use crate::checker::Checker;
use kai_ast::expr::{ApiContract, DslBlockExpr};
use kai_tast::{KaiType, TypedExpr, TypedExprKind};
use kai_diagnostics::Diagnostic;
use snapshot::{ApiSchema, ApiParameter};
use kai_diagnostics::Span;

pub(crate) fn check_api_block(checker: &mut Checker, expr: &DslBlockExpr, contract: &ApiContract) -> TypedExpr {
    let expected_ty = if let Some(ty_node) = &expr.return_ty {
        crate::ty::resolve(checker, ty_node)
    } else {
        KaiType::Unit
    };

    let snapshot_key = (contract.service.clone(), expr.version);
    let endpoint_key = format!("{} {}", contract.method.to_uppercase(), contract.path);
    
    let endpoint = {
        let snapshot = match checker.api_snapshots.get(&snapshot_key) {
            Some(s) => s,
            None => {
                checker.error(Diagnostic::error(
                    format!("api snapshot '{}' v{} not found", contract.service, expr.version),
                    expr.span,
                ));
                return TypedExpr::new(TypedExprKind::Invalid, expected_ty);
            }
        };

        match snapshot.endpoints.get(&endpoint_key).cloned() {
            Some(ep) => ep,
            None => {
                checker.error(Diagnostic::error(
                    format!("endpoint '{}' not found in API snapshot '{}' v{}", 
                            endpoint_key, contract.service, expr.version),
                    expr.span,
                ));
                return TypedExpr::new(TypedExprKind::Invalid, expected_ty);
            }
        }
    };


    // Check Auth
    if let Some(auth_expr) = &contract.auth {
        crate::expr::lower(checker, auth_expr, None);
    }

    // Check Body
    if let Some(body_fields) = &contract.body {
        if let Some(req_schema) = &endpoint.request_body {
            check_struct_literal(checker, body_fields, req_schema, expr.span, "body");
        } else {
            checker.error(Diagnostic::error(
                format!("endpoint '{}' does not expect a request body", endpoint_key),
                expr.span,
            ));
        }
    } else if let Some(schema) = &endpoint.request_body
        && is_schema_required(schema) {
            checker.error(Diagnostic::error(
                format!("endpoint '{}' requires a request body", endpoint_key),
                expr.span,
            ));
        }

    // Check Parameters
    check_parameters(checker, &contract.query_params, &endpoint.parameters, "query", expr.span);
    check_parameters(checker, &contract.path_params, &endpoint.parameters, "path", expr.span);
    check_parameters(checker, &contract.header_params, &endpoint.parameters, "header", expr.span);

    // Check Response
    if expected_ty != KaiType::Unit {
        if let Some(res_schema) = &endpoint.response {
            check_schema_match(checker, &expected_ty, res_schema, expr.span, "response");
        } else {
            checker.error(Diagnostic::error(
                format!("endpoint '{}' does not return a response, but contract expects {}", endpoint_key, expected_ty),
                expr.span,
            ));
        }
    }

    TypedExpr::new(TypedExprKind::Invalid, expected_ty)
}

fn check_struct_literal(checker: &mut Checker, fields: &[(kai_ast::Ident, kai_ast::expr::Expr)], schema: &ApiSchema, block_span: Span, block_name: &str) {
    if let ApiSchema::Object { properties, required, .. } = schema {
        let mut provided = std::collections::HashSet::new();
        
        for (ident, val_expr) in fields {
            provided.insert(ident.name.clone());
            let typed_val = crate::expr::lower(checker, val_expr, None);
            
            if let Some(prop_schema) = properties.get(&ident.name) {
                // Not doing full complex recursion for literals yet, just check primitive types
                if let ApiSchema::Primitive { kind, nullable, .. } = prop_schema {
                    let expected_kai = map_primitive_to_kai(kind);
                    let expected_kai = if *nullable { KaiType::Optional(Box::new(expected_kai)) } else { expected_kai };
                    
                    if typed_val.ty != expected_kai {
                        checker.error(Diagnostic::error(
                            format!("type mismatch: API field '{}' expects {}, but got {}", ident.name, expected_kai, typed_val.ty),
                            ident.span,
                        ));
                    }
                } else {
                    // For nested objects/arrays in request literals, we'd need to fully recurse, 
                    // which requires more complex literal checking logic. For V0.1 we just accept it if it compiles.
                }
            } else {
                checker.error(Diagnostic::error(
                    format!("unknown {} field '{}'", block_name, ident.name),
                    ident.span,
                ));
            }
        }
        
        for req_field in required {
            if !provided.contains(req_field) {
                checker.error(Diagnostic::error(
                    format!("missing required {} field '{}'", block_name, req_field),
                    block_span,
                ));
            }
        }
    } else {
        checker.error(Diagnostic::error(
            format!("{} must be an object", block_name),
            block_span,
        ));
    }
}

fn check_parameters(checker: &mut Checker, params_ast: &Option<Vec<(kai_ast::Ident, kai_ast::expr::Expr)>>, endpoint_params: &[ApiParameter], loc: &str, span: Span) {
    let mut expected_map = std::collections::HashMap::new();
    let mut required_set = std::collections::HashSet::new();
    
    for p in endpoint_params {
        if p.location == loc {
            expected_map.insert(p.name.clone(), p);
            if p.required {
                required_set.insert(p.name.clone());
            }
        }
    }

    if let Some(fields) = params_ast {
        let mut provided = std::collections::HashSet::new();
        for (ident, val_expr) in fields {
            provided.insert(ident.name.clone());
            let typed_val = crate::expr::lower(checker, val_expr, None);
            
            if let Some(p) = expected_map.get(&ident.name) {
                if let ApiSchema::Primitive { kind, .. } = &p.schema {
                    let expected_kai = map_primitive_to_kai(kind);
                    if typed_val.ty != expected_kai {
                        checker.error(Diagnostic::error(
                            format!("type mismatch: parameter '{}' in {} expects {}, got {}", ident.name, loc, expected_kai, typed_val.ty),
                            ident.span,
                        ));
                    }
                }
            } else {
                checker.error(Diagnostic::error(
                    format!("unknown {} parameter '{}'", loc, ident.name),
                    ident.span,
                ));
            }
        }
        
        for req_field in required_set {
            if !provided.contains(&req_field) {
                checker.error(Diagnostic::error(
                    format!("missing required {} parameter '{}'", loc, req_field),
                    span,
                ));
            }
        }
    } else if !required_set.is_empty() {
        checker.error(Diagnostic::error(
            format!("missing required {} parameters", loc),
            span,
        ));
    }
}

fn check_schema_match(checker: &mut Checker, kai_ty: &KaiType, schema: &ApiSchema, span: Span, ctx: &str) {
    match (kai_ty, schema) {
        (KaiType::Struct(id), ApiSchema::Object { properties, .. }) => {
            let layout = checker.structs[id.0 as usize].clone();
            for field in &layout.fields {
                if let Some(prop_schema) = properties.get(&field.name) {
                    check_schema_match(checker, &field.ty, prop_schema, span, &format!("{}.{}", ctx, field.name));
                } else {
                    checker.error(Diagnostic::error(
                        format!("response mismatch: API response does not contain field '{}' expected by struct '{}'", field.name, layout.name),
                        span,
                    ));
                }
            }
        },
        (KaiType::Array(inner), ApiSchema::Array { items, .. }) => {
            check_schema_match(checker, inner, items, span, &format!("{}[]", ctx));
        },
        (KaiType::Optional(inner), schema) => {
            let schema_nullable = match schema {
                ApiSchema::Primitive { nullable, .. } => *nullable,
                ApiSchema::Object { nullable, .. } => *nullable,
                ApiSchema::Array { nullable, .. } => *nullable,
                ApiSchema::Enum { nullable, .. } => *nullable,
            };
            if !schema_nullable {
                checker.error(Diagnostic::error(
                    format!("API schema for '{}' is not nullable, but Kai type is optional", ctx),
                    span,
                ));
            }
            check_schema_match(checker, inner, schema, span, ctx);
        },
        (ty, ApiSchema::Primitive { kind, .. }) => {
            let expected_kai = map_primitive_to_kai(kind);
            if ty != &expected_kai {
                checker.error(Diagnostic::error(
                    format!("type mismatch at {}: expected {}, got {}", ctx, expected_kai, ty),
                    span,
                ));
            }
        },
        _ => {
            checker.error(Diagnostic::error(
                format!("shape mismatch at {}: API schema is {:?} but Kai type is {}", ctx, schema, kai_ty),
                span,
            ));
        }
    }
}

fn map_primitive_to_kai(s: &str) -> KaiType {
    match s {
        "int32" => KaiType::Int32,
        "int64" => KaiType::Int64,
        "float32" => KaiType::Float64,
        "float64" => KaiType::Float64,
        "bool" => KaiType::Bool,
        "string" => KaiType::String,
        _ => KaiType::String,
    }
}

fn is_schema_required(schema: &ApiSchema) -> bool {
    match schema {
        ApiSchema::Primitive { nullable, .. } => !nullable,
        ApiSchema::Object { required, nullable, .. } => !nullable && !required.is_empty(),
        ApiSchema::Array { nullable, .. } => !nullable,
        ApiSchema::Enum { nullable, .. } => !nullable,
    }
}
