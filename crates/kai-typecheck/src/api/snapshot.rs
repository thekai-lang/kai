use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ApiSchema {
    Primitive { 
        kind: String, 
        format: Option<String>, 
        nullable: bool 
    },
    Object { 
        properties: HashMap<String, ApiSchema>,
        required: Vec<String>,
        nullable: bool
    },
    Array { 
        items: Box<ApiSchema>,
        nullable: bool
    },
    Enum {
        kind: String,
        variants: Vec<String>,
        nullable: bool
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiParameter {
    pub name: String,
    pub location: String, // "query", "path", "header"
    pub required: bool,
    pub schema: ApiSchema,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub parameters: Vec<ApiParameter>,
    pub request_body: Option<ApiSchema>,
    pub response: Option<ApiSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiSnapshot {
    pub version: u32,
    pub source: Option<String>,
    #[serde(default)]
    pub servers: Vec<String>,
    pub endpoints: HashMap<String, ApiEndpoint>,
}

pub fn parse_snapshot(s: &str) -> Result<ApiSnapshot, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}
