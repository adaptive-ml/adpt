use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct JsonSchema {
    pub properties: HashMap<String, JsonSchemaPropertyContents>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum JsonSchemaPropertyContents {
    Regular(RegularJsonSchemaPropertyContents),
    Union(UnionJsonSchemaPropertyContents),
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnionJsonSchemaPropertyContents {
    #[serde(rename = "oneOf")]
    one_of: Vec<JsonSchema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegularJsonSchemaPropertyContents {
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
}
