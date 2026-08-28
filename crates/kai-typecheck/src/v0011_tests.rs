use crate::api::snapshot::{ApiSnapshot, ApiEndpoint, ApiSchema, ApiParameter};
use std::collections::HashMap;

fn mock_api_snapshot() -> ApiSnapshot {
    let mut endpoints = HashMap::new();
    
    // POST /payment_intents
    let mut req_fields = HashMap::new();
    req_fields.insert("amount".to_string(), ApiSchema::Primitive { kind: "int32".to_string(), format: None, nullable: false }); 
    req_fields.insert("currency".to_string(), ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false });

    let mut res_fields = HashMap::new();
    res_fields.insert("id".to_string(), ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false });
    res_fields.insert("status".to_string(), ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false });

    endpoints.insert("POST /payment_intents".to_string(), ApiEndpoint {
        parameters: vec![],
        request_body: Some(ApiSchema::Object {
            properties: req_fields,
            required: vec!["amount".to_string(), "currency".to_string()],
            nullable: false,
        }),
        response: Some(ApiSchema::Object {
            properties: res_fields,
            required: vec!["id".to_string(), "status".to_string()],
            nullable: false,
        }),
    });

    ApiSnapshot {
        version: 3,
        source: Some("stripe".to_string()),
        servers: vec![],
        endpoints,
    }
}

fn assert_api_check(source: &str, expected_errors: &[&str]) {
    let sql_snaps = HashMap::new();
    let mut api_snaps = HashMap::new();
    
    api_snaps.insert(("stripe".to_string(), 3), mock_api_snapshot());

    let res = crate::test_support::check_source_with_all_snapshots(source, sql_snaps, api_snaps);
    
    if expected_errors.is_empty() {
        if let Err(diags) = res {
            panic!("Expected success, got errors: {:#?}", diags);
        }
    } else {
        match res {
            Ok(_) => panic!("Expected errors, but check succeeded"),
            Err(diags) => {
                let msgs: Vec<String> = diags.into_iter().map(|d| d.message).collect();
                for expected in expected_errors {
                    assert!(
                        msgs.iter().any(|m| m.contains(expected)),
                        "Expected error containing '{}', but got: {:#?}",
                        expected,
                        msgs
                    );
                }
            }
        }
    }
}

#[test]
fn test_api_valid_request() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: {
            amount: 2000,
            currency: "usd"
        }
    };
    return 0;
}
"#;
    assert_api_check(src, &[]);
}

#[test]
fn test_api_missing_field() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: {
            amount: 2000
        }
    };
    return 0;
}
"#;
    assert_api_check(src, &["missing required body field 'currency'"]);
}

#[test]
fn test_api_type_mismatch() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: {
            amount: "lots of money",
            currency: "usd"
        }
    };
    return 0;
}
"#;
    assert_api_check(src, &["type mismatch: API field 'amount' expects int32, but got string"]);
}

#[test]
fn test_api_response_mismatch() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: int;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: {
            amount: 2000,
            currency: "usd"
        }
    };
    return 0;
}
"#;
    assert_api_check(src, &["type mismatch at response.status: expected string, got int32"]);
}

#[test]
fn test_api_unknown_endpoint() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        GET /payment_intents
        with body: {
            amount: 2000,
            currency: "usd"
        }
    };
    return 0;
}
"#;
    assert_api_check(src, &["endpoint 'GET /payment_intents' not found in API snapshot 'stripe' v3"]);
}

#[test]
fn test_api_parameters_valid() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with query: { expand: "customer" }
        with header: { "Idempotency-Key": "req_123" }
        with auth: bearer("sk_123")
        with body: { amount: 2000, currency: "usd" }
    };
    return 0;
}
"#;
    
    // Inject parameters to the mock
    let mut api_snaps = HashMap::new();
    let mut snap = mock_api_snapshot();
    let ep = snap.endpoints.get_mut("POST /payment_intents").unwrap();
    ep.parameters.push(ApiParameter {
        name: "expand".to_string(),
        location: "query".to_string(),
        required: true,
        schema: ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false }
    });
    ep.parameters.push(ApiParameter {
        name: "Idempotency-Key".to_string(),
        location: "header".to_string(),
        required: false,
        schema: ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false }
    });
    api_snaps.insert(("stripe".to_string(), 3), snap);
    
    let res = crate::test_support::check_source_with_all_snapshots(src, HashMap::new(), api_snaps);
    if let Err(diags) = res {
        panic!("Expected success, got errors: {:#?}", diags);
    }
}

#[test]
fn test_api_parameters_missing() {
    let src = r#"
type PaymentIntent = {
    id: string;
    status: string;
}
fn bearer(token: string) -> string {
    return token;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: { amount: 2000, currency: "usd" }
    };
    return 0;
}
"#;
    
    let mut api_snaps = HashMap::new();
    let mut snap = mock_api_snapshot();
    let ep = snap.endpoints.get_mut("POST /payment_intents").unwrap();
    ep.parameters.push(ApiParameter {
        name: "expand".to_string(),
        location: "query".to_string(),
        required: true, // Missing this should error!
        schema: ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false }
    });
    api_snaps.insert(("stripe".to_string(), 3), snap);
    
    let res = crate::test_support::check_source_with_all_snapshots(src, HashMap::new(), api_snaps);
    
    match res {
        Ok(_) => panic!("Expected errors, but check succeeded"),
        Err(diags) => {
            let msgs: Vec<String> = diags.into_iter().map(|d| d.message).collect();
            assert!(msgs.iter().any(|m| m.contains("missing required query parameters")), "Got: {:?}", msgs);
        }
    }
}

#[test]
fn test_api_nested_response() {
    let src = r#"
type Metadata = {
    key: string;
}
type PaymentIntent = {
    id: string;
    metadata: Metadata;
}
fn main() -> int {
    let result = dsl api("stripe", v3) -> PaymentIntent {
        POST /payment_intents
        with body: { amount: 2000, currency: "usd" }
    };
    return 0;
}
"#;
    let mut api_snaps = HashMap::new();
    let mut snap = mock_api_snapshot();
    let ep = snap.endpoints.get_mut("POST /payment_intents").unwrap();
    
    // Add nested metadata to response schema
    if let Some(ApiSchema::Object { properties, required, .. }) = &mut ep.response {
        let mut meta_props = HashMap::new();
        meta_props.insert("key".to_string(), ApiSchema::Primitive { kind: "string".to_string(), format: None, nullable: false });
        properties.insert("metadata".to_string(), ApiSchema::Object {
            properties: meta_props,
            required: vec!["key".to_string()],
            nullable: false,
        });
        required.push("metadata".to_string());
    }
    
    api_snaps.insert(("stripe".to_string(), 3), snap);
    
    let res = crate::test_support::check_source_with_all_snapshots(src, HashMap::new(), api_snaps);
    if let Err(diags) = res {
        panic!("Expected success, got errors: {:#?}", diags);
    }
}
