//! v2 Admin Registry, ordinary artifact references, and typed provider bindings.

use serde::{Deserialize, Serialize};

pub const ADMIN_RESOURCE_API_VERSION: &str = "mei-admin-resource-v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdminRegistryEntry {
    pub api_version: String,
    pub app_id: String,
    pub resource_id: String,
    pub module_id: String,
    pub resource_key: String,
    pub canonical_route: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<AdminNavigation>,
    pub required_capabilities: Vec<String>,
    pub scope: String,
    pub audit: bool,
    pub danger_level: AdminDangerLevel,
    pub source_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminNavigation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AdminDangerLevel {
    #[default]
    Normal,
    Elevated,
    Critical,
}

impl AdminDangerLevel {
    pub(crate) fn parse(value: Option<&str>) -> Self {
        match value {
            Some("elevated") => Self::Elevated,
            Some("critical") => Self::Critical,
            _ => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminApplyPolicy {
    Hot,
    ReloadView,
    RestartRuntime,
}

impl AdminApplyPolicy {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "hot" => Some(Self::Hot),
            "reload-view" | "reload_view" => Some(Self::ReloadView),
            "restart-runtime" | "restart_runtime" => Some(Self::RestartRuntime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPayloadType {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderValidator {
    pub kind: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBinding {
    pub binding_id: String,
    pub provider_id: String,
    pub method: String,
    pub target: String,
    pub payload_type: ProviderPayloadType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<ProviderValidator>,
    pub revision: String,
    pub idempotency: String,
    pub apply_policy: AdminApplyPolicy,
    pub danger: AdminDangerLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<String>,
    pub source_anchor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminArtifactRef {
    pub artifact_id: String,
    pub content_hash: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminArtifactRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_full: Option<AdminArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_plans: Option<AdminArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_manifest: Option<AdminArtifactRef>,
}
