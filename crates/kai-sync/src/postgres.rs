use std::collections::HashMap;
use postgres::{Client, NoTls};
use serde_json::json;

pub fn sync_schema(version: u32, conn_str: &str, output_path: &str) -> Result<(), String> {
    let mut client = Client::connect(conn_str, NoTls)
        .map_err(|e| format!("Failed to connect to Postgres: {}", e))?;

    // Query information schema
    let query = "
        SELECT table_name, column_name, data_type 
        FROM information_schema.columns 
        WHERE table_schema = 'public'
    ";
    
    let mut schema: HashMap<String, HashMap<String, String>> = HashMap::new();

    for row in client.query(query, &[]).map_err(|e| format!("Query failed: {}", e))? {
        let table: String = row.get("table_name");
        let column: String = row.get("column_name");
        let data_type: String = row.get("data_type");

        let mapped_type = map_pg_type(&data_type);
        
        schema.entry(table).or_default().insert(column, mapped_type);
    }

    let snapshot = json!({
        "version": version,
        "source": { "kind": "postgres" },
        "schema": schema
    });

    let json_str = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

    std::fs::write(output_path, json_str)
        .map_err(|e| format!("Failed to write snapshot file: {}", e))?;

    Ok(())
}

fn map_pg_type(pg_type: &str) -> String {
    match pg_type.to_lowercase().as_str() {
        "integer" | "int4" => "int32".to_string(),
        "bigint" | "int8" => "int64".to_string(),
        "smallint" | "int2" => "int32".to_string(),
        "boolean" | "bool" => "bool".to_string(),
        "character varying" | "varchar" | "text" | "character" | "uuid" => "string".to_string(),
        "real" | "float4" | "double precision" | "float8" | "numeric" => "float64".to_string(),
        _ => "string".to_string(), // fallback
    }
}
