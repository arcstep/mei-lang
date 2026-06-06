use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config_refs::source_decl_from_value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDecl {
    #[serde(alias = "__source")]
    pub kind: String,
    #[serde(default)]
    #[serde(alias = "file")]
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default)]
    pub header_row: Option<i64>,
    #[serde(default)]
    pub preview_rows: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
    #[serde(default)]
    pub max_page_size: Option<i64>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub connection: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDecl {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_source_decl")]
    pub source: Option<SourceDecl>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub dataset: Option<Value>,
    #[serde(default)]
    pub metrics: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub filters: Option<Value>,
    /// Authoring-only：`resource(base = *_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
}

fn deserialize_optional_source_decl<'de, D>(
    deserializer: D,
) -> Result<Option<SourceDecl>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(value) => source_decl_from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedResource {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub document: Option<String>,
    #[serde(default)]
    pub dataset: Option<super::dataset::DatasetView>,
}
