use serde_json::{json, Value as JsonValue};
use serde_yaml::Value;
use std::collections::HashMap;

struct DocStore {
    cache: HashMap<String, Value>,
}

impl DocStore {
    fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    fn get_or_fetch(&mut self, url: &str) -> Result<&Value, String> {
        if !self.cache.contains_key(url) {
            let content = if url.starts_with("http") {
                let client = reqwest::blocking::Client::new();
                client.get(url).send()
                    .map_err(|e| format!("Failed to fetch URL: {}", e))?
                    .text()
                    .map_err(|e| format!("Failed to read response text: {}", e))?
            } else {
                std::fs::read_to_string(url)
                    .map_err(|e| format!("Failed to read local file {}: {}", url, e))?
            };
            
            let doc: Value = serde_yaml::from_str(&content)
                .map_err(|e| format!("Failed to parse YAML: {}", e))?;
                
            self.cache.insert(url.to_string(), doc);
        }
        Ok(self.cache.get(url).unwrap())
    }
}

pub fn sync_openapi(version: u32, source: &str, service_name: &str, output_path: &str) -> Result<(), String> {
    let mut store = DocStore::new();
    // Pre-seed the cache if it's the main document
    let main_doc = store.get_or_fetch(source)?.clone();

    let paths = main_doc.get("paths")
        .and_then(|v| v.as_mapping())
        .ok_or_else(|| "No `paths` object found in OpenAPI spec".to_string())?;
        
    let mut root_servers = Vec::new();
    if let Some(servers) = main_doc.get("servers").and_then(|v| v.as_sequence()) {
        for s in servers {
            if let Some(url) = s.get("url").and_then(|u| u.as_str()) {
                root_servers.push(url.to_string());
            }
        }
    }

    let mut endpoints = HashMap::new();

    for (path_key, path_item) in paths {
        let path_str = path_key.as_str().unwrap_or("");
        
        let path_item = resolve_ref(path_item.clone(), source, &mut store);
        
        if let Some(methods_map) = path_item.as_mapping() {
            let mut path_level_params = Vec::new();
            if let Some(params) = methods_map.get(Value::String("parameters".to_string()))
                && let Some(seq) = params.as_sequence() {
                    for p in seq {
                        path_level_params.push(p.clone());
                    }
                }

            for (method_key, operation) in methods_map {
                let method_str = method_key.as_str().unwrap_or("").to_uppercase();
                if !["GET", "POST", "PUT", "DELETE", "PATCH"].contains(&method_str.as_str()) {
                    continue;
                }
                
                let operation = resolve_ref(operation.clone(), source, &mut store);

                let endpoint_key = format!("{} {}", method_str, path_str);
                
                let mut all_params = path_level_params.clone();
                if let Some(op_params) = operation.get("parameters").and_then(|v| v.as_sequence()) {
                    for p in op_params {
                        all_params.push(p.clone());
                    }
                }
                
                let parameters = extract_parameters(&all_params, source, &mut store);
                let request_body = extract_request_schema(&operation, source, &mut store);
                let response = extract_response_schema(&operation, source, &mut store);

                endpoints.insert(endpoint_key, json!({
                    "parameters": parameters,
                    "request_body": request_body,
                    "response": response
                }));
            }
        }
    }

    let snapshot = json!({
        "version": version,
        "source": service_name,
        "servers": root_servers,
        "endpoints": endpoints
    });

    let json_str = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

    std::fs::write(output_path, json_str)
        .map_err(|e| format!("Failed to write snapshot file: {}", e))?;

    Ok(())
}

fn resolve_ref(mut value: Value, current_url: &str, store: &mut DocStore) -> Value {
    // Handle up to 10 levels of indirection to prevent infinite loops
    for _ in 0..10 {
        if let Some(ref_str) = value.get("$ref").and_then(|v| v.as_str()) {
            if ref_str.starts_with("#/") {
                // Local reference
                let parts: Vec<&str> = ref_str[2..].split('/').collect();
                if let Ok(doc) = store.get_or_fetch(current_url) {
                    let mut current = doc;
                    let mut found = true;
                    for part in parts {
                        if let Some(next) = current.get(part) {
                            current = next;
                        } else {
                            found = false;
                            break;
                        }
                    }
                    if found {
                        value = current.clone();
                        continue;
                    }
                }
            } else if ref_str.contains('#') {
                // External reference with fragment
                let parts: Vec<&str> = ref_str.splitn(2, '#').collect();
                let url = parts[0];
                let fragment = parts.get(1).unwrap_or(&"");
                
                if let Ok(ext_doc) = store.get_or_fetch(url) {
                    let mut current = ext_doc;
                    let mut found = true;
                    if fragment.starts_with('/') {
                        let path_parts: Vec<&str> = fragment[1..].split('/').collect();
                        for part in path_parts {
                            if let Some(next) = current.get(part) {
                                current = next;
                            } else {
                                found = false;
                                break;
                            }
                        }
                    }
                    if found {
                        value = current.clone();
                        continue; // Maybe the resolved value is ALSO a ref!
                    }
                }
            } else {
                // External reference without fragment (entire document)
                if let Ok(ext_doc) = store.get_or_fetch(ref_str) {
                    value = ext_doc.clone();
                    continue;
                }
            }
        }
        break;
    }
    value
}

fn extract_parameters(params: &[Value], current_url: &str, store: &mut DocStore) -> Vec<JsonValue> {
    let mut res = Vec::new();
    for p in params {
        let p = resolve_ref(p.clone(), current_url, store);
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let in_loc = p.get("in").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let required = p.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let schema = if let Some(s) = p.get("schema") {
            extract_schema(s.clone(), current_url, store)
        } else {
            json!({ "type": "Primitive", "kind": "string", "format": null, "nullable": false })
        };
        
        if !name.is_empty() && !in_loc.is_empty() {
            res.push(json!({
                "name": name,
                "location": in_loc,
                "required": required,
                "schema": schema
            }));
        }
    }
    res
}

fn extract_request_schema(operation: &Value, current_url: &str, store: &mut DocStore) -> Option<JsonValue> {
    let req_body = resolve_ref(operation.get("requestBody")?.clone(), current_url, store);
    let content = req_body.get("content")?;
    let app_json = content.get("application/json")?;
    let schema = app_json.get("schema")?;
    
    Some(extract_schema(schema.clone(), current_url, store))
}

fn extract_response_schema(operation: &Value, current_url: &str, store: &mut DocStore) -> Option<JsonValue> {
    let responses = operation.get("responses")?.as_mapping()?;
    for (code, response) in responses {
        let code_str = code.as_str().unwrap_or("");
        if code_str.starts_with('2') || code_str == "default" {
            let response = resolve_ref(response.clone(), current_url, store);
            if let Some(content) = response.get("content")
                && let Some(app_json) = content.get("application/json")
                    && let Some(schema) = app_json.get("schema") {
                        return Some(extract_schema(schema.clone(), current_url, store));
                    }
        }
    }
    None
}

fn extract_schema(schema: Value, current_url: &str, store: &mut DocStore) -> JsonValue {
    let schema = resolve_ref(schema, current_url, store);
    
    let t = schema.get("type").and_then(|v| v.as_str()).unwrap_or("object");
    let nullable = schema.get("nullable").and_then(|v| v.as_bool()).unwrap_or(false);
    
    match t {
        "array" => {
            let items = schema.get("items").unwrap_or(&Value::Null);
            json!({
                "type": "Array",
                "items": extract_schema(items.clone(), current_url, store),
                "nullable": nullable
            })
        },
        "object" => {
            let mut props_map = serde_json::Map::new();
            if let Some(props) = schema.get("properties").and_then(|p| p.as_mapping()) {
                for (k, v) in props {
                    if let Some(key_str) = k.as_str() {
                        props_map.insert(key_str.to_string(), extract_schema(v.clone(), current_url, store));
                    }
                }
            }
            let mut required_vec = Vec::new();
            if let Some(req) = schema.get("required").and_then(|v| v.as_sequence()) {
                for r in req {
                    if let Some(rs) = r.as_str() {
                        required_vec.push(rs.to_string());
                    }
                }
            }
            json!({
                "type": "Object",
                "properties": props_map,
                "required": required_vec,
                "nullable": nullable
            })
        },
        _ => {
            let f = schema.get("format").and_then(|v| v.as_str());
            let kind = match (t, f) {
                ("integer", Some("int64")) => "int64",
                ("integer", _) => "int32",
                ("number", Some("float")) => "float32",
                ("number", _) => "float64",
                ("boolean", _) => "bool",
                ("string", _) => "string",
                _ => "string",
            };
            json!({
                "type": "Primitive",
                "kind": kind,
                "format": f,
                "nullable": nullable
            })
        }
    }
}
