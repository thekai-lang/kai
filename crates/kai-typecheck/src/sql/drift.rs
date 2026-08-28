use crate::sql::snapshot::SqlType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftSeverity {
    Error,
    Warning,
    Quiet,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriftKind {
    /// Expected snapshot is completely missing (this is technically not drift, but a missing baseline)
    MissingSnapshot,
    /// Column is removed in the current schema
    ColumnRemoved,
    /// Type changed in an incompatible way (e.g. Uuid -> Int)
    IncompatibleType { old: SqlType, new: SqlType },
    /// T became T?
    NonNullBecameNullable,
    /// T? became T
    NullableBecameNonNull,
}

#[derive(Debug, Clone)]
pub struct DriftRecord {
    pub target: String,
    pub kind: DriftKind,
    pub severity: DriftSeverity,
}

/// Evaluates if the current live type drifts from the expected type.
pub fn compare_column_drift(target: &str, expected: &SqlType, actual: Option<&SqlType>) -> Option<DriftRecord> {
    let actual = match actual {
        Some(ty) => ty,
        None => return Some(DriftRecord {
            target: target.to_string(),
            kind: DriftKind::ColumnRemoved,
            severity: DriftSeverity::Error,
        }),
    };

    // Fast path: identical types
    if expected == actual {
        return None;
    }

    match (expected, actual) {
        // T expected, T? actual -> Error
        (SqlType::Uuid, SqlType::Nullable(inner)) |
        (SqlType::String, SqlType::Nullable(inner)) |
        (SqlType::Int32, SqlType::Nullable(inner)) |
        (SqlType::Int64, SqlType::Nullable(inner)) |
        (SqlType::Float64, SqlType::Nullable(inner)) |
        (SqlType::Bool, SqlType::Nullable(inner)) => {
            if &**inner == expected {
                return Some(DriftRecord {
                    target: target.to_string(),
                    kind: DriftKind::NonNullBecameNullable,
                    severity: DriftSeverity::Error,
                });
            }
        }
        
        // T? expected, T actual -> Warning
        (SqlType::Nullable(inner), actual_base) => {
            if &**inner == actual_base {
                return Some(DriftRecord {
                    target: target.to_string(),
                    kind: DriftKind::NullableBecameNonNull,
                    severity: DriftSeverity::Warning,
                });
            }
        }
        _ => {}
    }

    // Check base incompatibility
    let exp_base = if let SqlType::Nullable(inner) = expected { &**inner } else { expected };
    let act_base = if let SqlType::Nullable(inner) = actual { &**inner } else { actual };

    if exp_base != act_base {
        return Some(DriftRecord {
            target: target.to_string(),
            kind: DriftKind::IncompatibleType {
                old: expected.clone(),
                new: actual.clone(),
            },
            severity: DriftSeverity::Error,
        });
    }

    None
}
