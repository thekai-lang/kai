use crate::sql::snapshot::SqlType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftSeverity {
    Error,
    Warning,
    Quiet,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlDriftKind {
    MissingSnapshot,
    ColumnRemoved,
    IncompatibleType { old: SqlType, new: SqlType },
    NonNullBecameNullable,
    NullableBecameNonNull,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiDriftKind {
    EndpointRemoved,
    FieldRemoved { field: String },
    TypeChanged { field: String, old: String, new: String },
    RequiredChanged { field: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriftKind {
    Sql(SqlDriftKind),
    Api(ApiDriftKind),
}

#[derive(Debug, Clone)]
pub struct DriftRecord {
    pub target: String,
    pub kind: DriftKind,
    pub severity: DriftSeverity,
}

pub fn compare_sql_column(target: &str, expected: &SqlType, actual: Option<&SqlType>) -> Option<DriftRecord> {
    let actual = match actual {
        Some(ty) => ty,
        None => return Some(DriftRecord {
            target: target.to_string(),
            kind: DriftKind::Sql(SqlDriftKind::ColumnRemoved),
            severity: DriftSeverity::Error,
        }),
    };

    if expected == actual {
        return None;
    }

    match (expected, actual) {
        (SqlType::Uuid, SqlType::Nullable(inner)) |
        (SqlType::String, SqlType::Nullable(inner)) |
        (SqlType::Int32, SqlType::Nullable(inner)) |
        (SqlType::Int64, SqlType::Nullable(inner)) |
        (SqlType::Float64, SqlType::Nullable(inner)) |
        (SqlType::Bool, SqlType::Nullable(inner)) => {
            if &**inner == expected {
                return Some(DriftRecord {
                    target: target.to_string(),
                    kind: DriftKind::Sql(SqlDriftKind::NonNullBecameNullable),
                    severity: DriftSeverity::Error,
                });
            }
        }
        (SqlType::Nullable(inner), actual_base)
            if &**inner == actual_base => {
                return Some(DriftRecord {
                    target: target.to_string(),
                    kind: DriftKind::Sql(SqlDriftKind::NullableBecameNonNull),
                    severity: DriftSeverity::Warning,
                });
            }
        _ => {}
    }

    let exp_base = if let SqlType::Nullable(inner) = expected { &**inner } else { expected };
    let act_base = if let SqlType::Nullable(inner) = actual { &**inner } else { actual };

    if exp_base != act_base {
        return Some(DriftRecord {
            target: target.to_string(),
            kind: DriftKind::Sql(SqlDriftKind::IncompatibleType {
                old: expected.clone(),
                new: actual.clone(),
            }),
            severity: DriftSeverity::Error,
        });
    }

    None
}
