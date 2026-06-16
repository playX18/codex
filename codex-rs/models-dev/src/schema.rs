use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelsDevCatalog {
    #[serde(flatten)]
    pub providers: HashMap<String, ModelsDevProvider>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelsDevProvider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    pub api: Option<String>,
    pub npm: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, ModelsDevModel>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelsDevModelCost {
    #[serde(default, deserialize_with = "deserialize_cost_value")]
    pub input: f64,
    #[serde(default, deserialize_with = "deserialize_cost_value")]
    pub output: f64,
    #[serde(default, deserialize_with = "deserialize_cost_value")]
    pub cache_read: f64,
    #[serde(default, deserialize_with = "deserialize_cost_value")]
    pub cache_write: f64,
}

fn deserialize_cost_value<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(match value {
        JsonValue::Number(number) => number.as_f64().unwrap_or_default(),
        JsonValue::Object(object) => object
            .values()
            .find_map(JsonValue::as_f64)
            .unwrap_or_default(),
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::String(_) | JsonValue::Array(_) => 0.0,
    })
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelsDevModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default)]
    pub temperature: bool,
    pub release_date: Option<String>,
    pub family: Option<String>,
    pub limit: ModelsDevModelLimit,
    pub status: Option<String>,
    pub cost: Option<ModelsDevModelCost>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelsDevModelLimit {
    pub context: u64,
    pub output: u64,
    pub input: Option<u64>,
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
