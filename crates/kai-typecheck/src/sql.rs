use crate::checker::Checker;
use kai_ast::expr::{DslBlockExpr, DslVariant};
use kai_tast::{KaiType, TypedExpr, TypedExprKind};
use kai_diagnostics::Diagnostic;
use std::collections::HashMap;

pub mod snapshot;


pub(crate) fn check_dsl_block(checker: &mut Checker, expr: &DslBlockExpr) -> TypedExpr {
    let expected_ty = if let Some(ty_node) = &expr.return_ty {
        crate::ty::resolve(checker, ty_node)
    } else {
        KaiType::Unit
    };

    if expr.kind != "sql" && expr.kind != "api" {
        checker.error(Diagnostic::error(
            format!("unsupported dsl kind: `{}` (only `sql` is supported in v0.0.10)", expr.kind),
            expr.span,
        ));
        return TypedExpr::new(TypedExprKind::Invalid, expected_ty);
    }

    match &expr.variant {
        DslVariant::StructuredApi(api_contract) => {
            return crate::api::check_api_block(checker, expr, api_contract);
        }
        DslVariant::StructuredSql(query) => {
            if let Some(tables) = checker.snapshots.get(&expr.version).map(|s| s.tables.clone()) {
                let mut sources = HashMap::new();
                let mut valid_sources = true;
                
                // Add FROM table
                if !query.from.name.is_empty() {
                    if let Some(table) = tables.get(&query.from.name) {
                        sources.insert(query.from.name.clone(), table.columns.clone());
                    } else {
                        checker.error(Diagnostic::error(
                            format!("table `{}` not found in snapshot v{}", query.from.name, expr.version),
                            query.from.span,
                        ));
                        valid_sources = false;
                    }
                }
                
                // Add JOIN tables
                for join in &query.joins {
                    if let Some(table) = tables.get(&join.table.name) {
                        sources.insert(join.table.name.clone(), table.columns.clone());
                    } else {
                        checker.error(Diagnostic::error(
                            format!("table `{}` not found in snapshot v{}", join.table.name, expr.version),
                            join.table.span,
                        ));
                        valid_sources = false;
                    }
                }

                
                // Validate ON conditions
                for join in &query.joins {
                    if let Some(ty) = resolve_sql_expr(&join.on_clause, &sources, expr.version, checker)
                        && unwrap_nullable(&ty) != &snapshot::SqlType::Bool {
                            checker.error(Diagnostic::error(
                                format!("JOIN ON condition must resolve to Bool, found {:?}", ty),
                                join.table.span,
                            ));
                        }
                }
                
                // Validate WHERE conditions
                if let Some(where_clause) = &query.where_clause
                    && let Some(ty) = resolve_sql_expr(where_clause, &sources, expr.version, checker)
                        && unwrap_nullable(&ty) != &snapshot::SqlType::Bool {
                            checker.error(Diagnostic::error(
                                format!("WHERE condition must resolve to Bool, found {:?}", ty),
                                expr.span,
                            ));
                        }
                
                // Validate ORDER BY conditions
                for order in &query.order_by {
                    let _ = resolve_sql_expr(&order.expr, &sources, expr.version, checker);
                }

                if valid_sources {
                    let mut element_ty = expected_ty.clone();
                    if let KaiType::Array(inner) = &element_ty {
                        element_ty = *inner.clone();
                    }
                    
                    match &element_ty {
                        KaiType::Struct(id) => {
                            let layout = checker.structs[id.0 as usize].clone();
                            
                            let mut seen_columns = std::collections::HashSet::new();
                            for select_expr in &query.select {
                                if let kai_ast::expr::SqlExpr::Column { qualifier, name, span } = &select_expr.expr {
                                    
                                    let effective_name = select_expr.alias.as_ref().unwrap_or(name).clone();
                                    
                                    // 1. Source resolution & Duplicate checking
                                    if !seen_columns.insert(effective_name.clone()) {
                                        checker.error(Diagnostic::error(
                                            format!("duplicate column `{}` in select list", effective_name),
                                            *span,
                                        ));
                                        continue;
                                    }

                                    let resolved_sql_ty = resolve_sql_column(name, qualifier, &sources, expr.version, *span, checker);
                                    // 2. Strict Extra Column validation & Type mapping
                                    if let Some(sql_ty) = resolved_sql_ty {
                                        if let Some(field) = layout.fields.iter().find(|f| f.name == effective_name) {
                                            if !is_compatible(&sql_ty, &field.ty) {
                                                checker.error(Diagnostic::error(
                                                    format!("type mismatch: SQL column `{}` is {:?}, but struct field `{}` expects {}", name, sql_ty, effective_name, field.ty),
                                                    *span,
                                                ));
                                            }
                                        } else {
                                            checker.error(Diagnostic::error(
                                                format!("query result selects extra column `{}` which does not exist in struct `{}`", effective_name, layout.name),
                                                *span,
                                            ));
                                        }
                                    }
                                }
                            }
                            
                            // Strict check: Ensure all struct fields were satisfied
                            for field in &layout.fields {
                                let mut found = false;
                                for select_expr in &query.select {
                                    if let kai_ast::expr::SqlExpr::Column { name, .. } = &select_expr.expr {
                                        let effective_name = select_expr.alias.as_ref().unwrap_or(name);
                                        if effective_name == &field.name {
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                                if !found {
                                    checker.error(Diagnostic::error(
                                        format!("query result is missing field `{}` required by `{}`", field.name, layout.name),
                                        expr.span,
                                    ));
                                }
                            }
                        }
                        _ => {
                            checker.error(Diagnostic::error(
                                format!("`dsl sql` returning multiple columns must be mapped to a Struct or Struct[] (found `{}`)", expected_ty),
                                expr.span,
                            ));
                        }
                    }
                }
            } else {
                checker.error(Diagnostic::error(
                    format!("schema snapshot v{} not found (run `kai db sync`?)", expr.version),
                    expr.span,
                ));
            }
        }
        DslVariant::Raw(_query_str) => {
            // Escape hatch: skip structural checks, blindly trust it produces expected_ty
        }
    }

    TypedExpr::new(TypedExprKind::Invalid, expected_ty)
}

fn is_compatible(sql: &snapshot::SqlType, kai: &KaiType) -> bool {
    match (sql, kai) {
        (snapshot::SqlType::Nullable(inner), KaiType::Optional(inner_kai)) => {
            is_compatible(inner, inner_kai)
        }
        (snapshot::SqlType::Nullable(_), _) => false, // Nullable SQL must map to Optional Kai
        (inner_sql, KaiType::Optional(inner_kai)) => {
            // Non-nullable SQL can optionally map to Optional Kai (widening)
            is_compatible(inner_sql, inner_kai)
        }
        (snapshot::SqlType::Uuid, KaiType::String) => true,
        (snapshot::SqlType::String, KaiType::String) => true,
        (snapshot::SqlType::Int32, KaiType::Int32) => true,
        (snapshot::SqlType::Int64, KaiType::Int64) => true,
        (snapshot::SqlType::Float64, KaiType::Float64) => true,
        (snapshot::SqlType::Bool, KaiType::Bool) => true,
        _ => false,
    }
}


fn resolve_sql_column(
    name: &str,
    qualifier: &Option<String>,
    sources: &HashMap<String, HashMap<String, snapshot::SqlType>>,
    version: u32,
    span: kai_diagnostics::Span,
    checker: &mut Checker,
) -> Option<snapshot::SqlType> {
    let resolved_sql_ty;
    let resolved_table;
    
    if let Some(q) = qualifier {
        if let Some(columns) = sources.get(q) {
            if let Some(sql_ty) = columns.get(name) {
                resolved_sql_ty = Some(sql_ty.clone());
                resolved_table = q.clone();
            } else {
                checker.error(Diagnostic::error(
                    format!("column `{}` not found in table `{}` (snapshot v{})", name, q, version),
                    span,
                ));
                return None;
            }
        } else {
            checker.error(Diagnostic::error(
                format!("invalid qualifier `{}`: table is not joined", q),
                span,
            ));
            return None;
        }
    } else {
        let mut found_in = Vec::new();
        for (src_name, columns) in sources {
            if let Some(sql_ty) = columns.get(name) {
                found_in.push((src_name.clone(), sql_ty.clone()));
            }
        }
        
        if found_in.len() == 1 {
            resolved_sql_ty = Some(found_in[0].1.clone());
            resolved_table = found_in[0].0.clone();
        } else if found_in.len() > 1 {
            checker.error(Diagnostic::error(
                format!("ambiguous column `{}`", name), // Changed string slightly based on what was there
                span,
            ));
            return None;
        } else {
            checker.error(Diagnostic::error(
                format!("column `{}` not found in any source tables (snapshot v{})", name, version),
                span,
            ));
            return None;
        }
    }
    
    // DRIFT ENGINE INTEGRATION
    if let Some(live) = &checker.current_schema {
        let actual = live.tables.get(&resolved_table).and_then(|t| t.columns.get(name));
        if let Some(drift) = crate::drift::compare_sql_column(name, resolved_sql_ty.as_ref().unwrap(), actual) {
            let msg = match drift.kind {
                crate::drift::DriftKind::Sql(crate::drift::SqlDriftKind::ColumnRemoved) => {
                    format!("column '{}' was removed from current schema", name)
                }
                crate::drift::DriftKind::Sql(crate::drift::SqlDriftKind::IncompatibleType { old, new }) => {
                    format!("column '{}' type changed from {:?} to {:?} in current schema", name, old, new)
                }
                crate::drift::DriftKind::Sql(crate::drift::SqlDriftKind::NonNullBecameNullable) => {
                    format!("column '{}' became nullable in current schema", name)
                }
                crate::drift::DriftKind::Sql(crate::drift::SqlDriftKind::NullableBecameNonNull) => {
                    format!("column '{}' became non-nullable in current schema", name)
                }
                crate::drift::DriftKind::Api(_) => unreachable!(),
                crate::drift::DriftKind::Sql(crate::drift::SqlDriftKind::MissingSnapshot) => {
                    "missing snapshot".to_string()
                }
            };
            
            let diag = match drift.severity {
                crate::drift::DriftSeverity::Error => Diagnostic::error(msg, span),
                crate::drift::DriftSeverity::Warning => Diagnostic::warning(msg, span),
                crate::drift::DriftSeverity::Quiet => Diagnostic::warning(msg, span), // Shouldn't happen
            };
            
            checker.error(diag);
        }
    }
    
    resolved_sql_ty
}

fn unwrap_nullable(ty: &snapshot::SqlType) -> &snapshot::SqlType {
    match ty {
        snapshot::SqlType::Nullable(inner) => inner.as_ref(),
        _ => ty,
    }
}


fn resolve_sql_expr(
    expr: &kai_ast::expr::SqlExpr,
    sources: &HashMap<String, HashMap<String, snapshot::SqlType>>,
    version: u32,
    checker: &mut Checker,
) -> Option<snapshot::SqlType> {
    match expr {
        kai_ast::expr::SqlExpr::Column { qualifier, name, span } => {
            resolve_sql_column(name, qualifier, sources, version, *span, checker)
        }
        kai_ast::expr::SqlExpr::IntLit { .. } => Some(snapshot::SqlType::Int64),
        kai_ast::expr::SqlExpr::StringLit { .. } => Some(snapshot::SqlType::String),
        kai_ast::expr::SqlExpr::BoolLit { .. } => Some(snapshot::SqlType::Bool),
        kai_ast::expr::SqlExpr::BinaryOp(lhs, op, rhs) => {
            let l_ty = resolve_sql_expr(lhs, sources, version, checker)?;
            let r_ty = resolve_sql_expr(rhs, sources, version, checker)?;
            
            let l_base = unwrap_nullable(&l_ty);
            let r_base = unwrap_nullable(&r_ty);
            
            let span = match &**lhs {
                kai_ast::expr::SqlExpr::Column { span, .. } => *span,
                kai_ast::expr::SqlExpr::IntLit { span, .. } => *span,
                kai_ast::expr::SqlExpr::StringLit { span, .. } => *span,
                kai_ast::expr::SqlExpr::BoolLit { span, .. } => *span,
                kai_ast::expr::SqlExpr::BinaryOp(inner, _, _) => match &**inner {
                    kai_ast::expr::SqlExpr::Column { span, .. } => *span,
                    _ => kai_diagnostics::Span::new(0, 0),
                },
                _ => kai_diagnostics::Span::new(0, 0),
            };

            match op {
                kai_ast::expr::SqlOp::Eq | kai_ast::expr::SqlOp::NotEq |
                kai_ast::expr::SqlOp::Lt | kai_ast::expr::SqlOp::Gt |
                kai_ast::expr::SqlOp::Le | kai_ast::expr::SqlOp::Ge => {
                    if l_base != r_base {
                        checker.error(Diagnostic::error(
                            format!("cannot compare {:?} with {:?} in condition", l_base, r_base),
                            span,
                        ));
                        return None;
                    }
                    Some(snapshot::SqlType::Bool)
                }
                kai_ast::expr::SqlOp::And | kai_ast::expr::SqlOp::Or => {
                    if l_base != &snapshot::SqlType::Bool || r_base != &snapshot::SqlType::Bool {
                        checker.error(Diagnostic::error(
                            format!("logical operators require Bool, found {:?} and {:?}", l_base, r_base),
                            span,
                        ));
                        return None;
                    }
                    Some(snapshot::SqlType::Bool)
                }
            }
        }
        kai_ast::expr::SqlExpr::Variable(_) => None, // Not supported yet
    }
}
