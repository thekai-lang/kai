use crate::check_with;
use crate::test_support::parse_ok;
use std::collections::HashMap;
use crate::sql::snapshot::{SqlSnapshot, SqlTable, SqlType};
use kai_resolver::{analyze_modules, ModuleInput};

fn setup_snapshots() -> HashMap<u32, SqlSnapshot> {
    let mut snapshots = HashMap::new();
    
    let mut users_cols = HashMap::new();
    users_cols.insert("id".to_string(), SqlType::Uuid);
    users_cols.insert("name".to_string(), SqlType::String);
    users_cols.insert("age".to_string(), SqlType::Nullable(Box::new(SqlType::Int32)));
    
    let mut orders_cols = HashMap::new();
    orders_cols.insert("id".to_string(), SqlType::Uuid);
    orders_cols.insert("user_id".to_string(), SqlType::Uuid);
    orders_cols.insert("total".to_string(), SqlType::Float64);
    
    let mut tables = HashMap::new();
    tables.insert("users".to_string(), SqlTable { columns: users_cols });
    tables.insert("orders".to_string(), SqlTable { columns: orders_cols });
    
    snapshots.insert(12, SqlSnapshot { version: 12, source_kind: None, source_database: None, captured_at: None, tables });
    snapshots
}

fn assert_err(source: &str, expected_msg: &str) {
    let ast = parse_ok(source);
    let inputs = vec![ModuleInput {
        name: "test",
        file: "test.kai",
        program: &ast,
    }];
    let resolution = analyze_modules(&inputs).unwrap();
    let result = check_with(&ast, &resolution, setup_snapshots(), std::collections::HashMap::new());
    
    match result {
        Ok(_) => panic!("Expected error '{}', but got success", expected_msg),
        Err(diags) => {
            let msg = &diags[0].message;
            assert!(msg.contains(expected_msg), "Expected error '{}', got: {}", expected_msg, msg);
        }
    }
}

fn assert_ok(source: &str) {
    let ast = parse_ok(source);
    let inputs = vec![ModuleInput {
        name: "test",
        file: "test.kai",
        program: &ast,
    }];
    let resolution = analyze_modules(&inputs).unwrap();
    let result = check_with(&ast, &resolution, setup_snapshots(), std::collections::HashMap::new());
    if let Err(diags) = result {
        panic!("Expected ok, got errors: {:?}", diags);
    }
}

#[test]
fn test_missing_snapshot() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v999) -> User { select id from users }; return 0; }";
    assert_err(src, "schema snapshot v999 not found");
}

#[test]
fn test_missing_table() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id from roles }; return 0; }";
    assert_err(src, "table `roles` not found in snapshot v12");
}

#[test]
fn test_missing_column() {
    let src = "type User = { password: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select users.password from users }; return 0; }";
    assert_err(src, "column `password` not found in table `users`");
}

#[test]
fn test_type_mismatch() {
    let src = "type User = { id: int32; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id from users }; return 0; }";
    assert_err(src, "type mismatch: SQL column `id` is Uuid, but struct field `id` expects int32");
}

#[test]
fn test_missing_field_in_result() {
    let src = "type User = { id: string; name: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id from users }; return 0; }";
    assert_err(src, "query result is missing field `name` required by `User`");
}

#[test]
fn test_extra_column_in_result() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, name from users }; return 0; }";
    assert_err(src, "query result selects extra column `name` which does not exist in struct `User`");
}

#[test]
fn test_duplicate_column() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, id from users }; return 0; }";
    assert_err(src, "duplicate column `id` in select list");
}

#[test]
fn test_invalid_qualifier() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select orders.id from users }; return 0; }";
    assert_err(src, "invalid qualifier `orders`: table is not joined");
}

#[test]
fn test_valid_qualifier() {
    let src = "type User = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select users.id from users }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_nullable_type_mismatch() {
    let src = "type User = { id: string; age: int32; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, age from users }; return 0; }";
    assert_err(src, "type mismatch: SQL column `age` is Nullable(Int32), but struct field `age` expects int32");
}

#[test]
fn test_nullable_type_match() {
    let src = "type User = { id: string; age: int32?; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, age from users }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_widening_nonnull_to_optional() {
    let src = "type User = { id: string?; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id from users }; return 0; }";
    assert_ok(src);
}


#[test]
fn test_valid_join() {
    let src = "type UserOrder = { id: string; name: string; total: float64; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.id, users.name, orders.total from users join orders on users.id == orders.user_id }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_ambiguous_column() {
    // Both users and orders have an `id` column
    let src = "type UserOrder = { id: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select id from users join orders on users.id == orders.user_id }; return 0; }";
    assert_err(src, "ambiguous column `id`");
}

#[test]
fn test_unqualified_column_success() {
    // Only users has `name` column, so unqualified `name` is fine
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select name from users join orders on users.id == orders.user_id }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_missing_join_table() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select name from users join items on users.id == items.user_id }; return 0; }";
    assert_err(src, "table `items` not found in snapshot v12");
}


#[test]
fn test_join_on_type_mismatch() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users join orders on users.id == orders.total }; return 0; }";
    assert_err(src, "cannot compare Uuid with Float64 in condition");
}

#[test]
fn test_join_on_invalid_column() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users join orders on users.foo == orders.user_id }; return 0; }";
    assert_err(src, "column `foo` not found in table `users`");
}


#[test]
fn test_where_valid() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users where users.name == \"some-name\" }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_where_non_bool() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users where users.id }; return 0; }";
    assert_err(src, "WHERE condition must resolve to Bool, found Uuid");
}

#[test]
fn test_join_literal_type_mismatch() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users join orders on users.id == 123 }; return 0; }";
    assert_err(src, "cannot compare Uuid with Int64 in condition");
}


#[test]
fn test_order_by_valid() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select name from users order by name desc }; return 0; }";
    assert_ok(src);
}

#[test]
fn test_order_by_invalid_column() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select name from users order by invalid_col asc }; return 0; }";
    assert_err(src, "column `invalid_col` not found in any source tables");
}

#[test]
fn test_order_by_limit_valid() {
    let src = "type UserOrder = { name: string; } fn main() -> int32 { let x = dsl sql(v12) -> UserOrder { select users.name from users where users.name == \"John\" order by users.name desc limit 10 }; return 0; }";
    assert_ok(src);
}



fn assert_drift(source: &str, expected_msg: Option<&str>, live_snap: crate::sql::snapshot::SqlSnapshot) {
    let ast = parse_ok(source);
    let inputs = vec![kai_resolver::ModuleInput {
        name: "test",
        file: "test.kai",
        program: &ast,
    }];
    let resolution = kai_resolver::analyze_modules(&inputs).unwrap();
    let result = crate::check_with_schema(&ast, &resolution, setup_snapshots(), std::collections::HashMap::new(), Some(live_snap));
    
    match (result, expected_msg) {
        (Ok(_), None) => {},
        (Ok(_), Some(msg)) => panic!("Expected error '{}', got success", msg),
        (Err(diags), None) => panic!("Expected success, got errors: {:?}", diags),
        (Err(diags), Some(expected_msg)) => {
            let msg = &diags[0].message;
            assert!(msg.contains(expected_msg), "Expected error '{}', got: {}", expected_msg, msg);
        }
    }
}

#[test]
fn test_drift_column_removed() {
    let src = "type User = { id: string; name: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, name from users }; return 0; }";
    let mut live_snap = crate::sql::snapshot::SqlSnapshot {
        version: 15, source_kind: None, source_database: None, captured_at: None, tables: std::collections::HashMap::new(),
    };
    let mut live_cols = std::collections::HashMap::new();
    live_cols.insert("id".to_string(), crate::sql::snapshot::SqlType::Uuid);
    live_snap.tables.insert("users".to_string(), crate::sql::snapshot::SqlTable { columns: live_cols });
    
    assert_drift(src, Some("column 'name' was removed from current schema"), live_snap);
}

#[test]
fn test_drift_incompatible_type() {
    let src = "type User = { id: string; name: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, name from users }; return 0; }";
    let mut live_snap = crate::sql::snapshot::SqlSnapshot {
        version: 15, source_kind: None, source_database: None, captured_at: None, tables: std::collections::HashMap::new(),
    };
    let mut live_cols = std::collections::HashMap::new();
    live_cols.insert("id".to_string(), crate::sql::snapshot::SqlType::Uuid);
    live_cols.insert("name".to_string(), crate::sql::snapshot::SqlType::Int32);
    live_snap.tables.insert("users".to_string(), crate::sql::snapshot::SqlTable { columns: live_cols });
    
    assert_drift(src, Some("column 'name' type changed from String to Int32"), live_snap);
}

#[test]
fn test_drift_nullable_became_non_null() {
    // Note: the test setup has "name" as String not Nullable(String), but in v12 it's just String.
    // wait, setup_snapshots sets users.name = SqlType::String (non-null).
    // So to test non-null -> nullable we just use the default snapshot.
    let src = "type User = { id: string; name: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, name from users }; return 0; }";
    let mut live_snap = crate::sql::snapshot::SqlSnapshot {
        version: 15, source_kind: None, source_database: None, captured_at: None, tables: std::collections::HashMap::new(),
    };
    let mut live_cols = std::collections::HashMap::new();
    live_cols.insert("id".to_string(), crate::sql::snapshot::SqlType::Uuid);
    live_cols.insert("name".to_string(), crate::sql::snapshot::SqlType::Nullable(Box::new(crate::sql::snapshot::SqlType::String)));
    live_snap.tables.insert("users".to_string(), crate::sql::snapshot::SqlTable { columns: live_cols });
    
    assert_drift(src, Some("column 'name' became nullable in current schema"), live_snap);
}

#[test]
fn test_drift_quiet_unused_addition() {
    let src = "type User = { id: string; name: string; } fn main() -> int32 { let x = dsl sql(v12) -> User { select id, name from users }; return 0; }";
    let mut live_snap = crate::sql::snapshot::SqlSnapshot {
        version: 15, source_kind: None, source_database: None, captured_at: None, tables: std::collections::HashMap::new(),
    };
    let mut live_cols = std::collections::HashMap::new();
    live_cols.insert("id".to_string(), crate::sql::snapshot::SqlType::Uuid);
    live_cols.insert("name".to_string(), crate::sql::snapshot::SqlType::String);
    live_cols.insert("phone".to_string(), crate::sql::snapshot::SqlType::String); // NEW unused
    live_snap.tables.insert("users".to_string(), crate::sql::snapshot::SqlTable { columns: live_cols });
    
    assert_drift(src, None, live_snap);
}
