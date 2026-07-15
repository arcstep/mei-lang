use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceState {
    Bundled,
    External,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceSeverity {
    Blocking,
    Degrade,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntry {
    pub id: String,
    pub app_id: String,
    pub kind: String,
    pub state: ResourceState,
    /// Workspace-relative target path after materialize, e.g. `apps/zhifa/upload/a.xlsx`.
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_for: Option<String>,
    pub severity: ResourceSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesDocument {
    pub schema_version: String,
    #[serde(default)]
    pub resources: Vec<ResourceEntry>,
}

impl ResourcesDocument {
    pub const SCHEMA: &'static str = "mei-snapshot-resources-v1";

    pub fn new(resources: Vec<ResourceEntry>) -> Self {
        Self {
            schema_version: Self::SCHEMA.to_string(),
            resources,
        }
    }
}
