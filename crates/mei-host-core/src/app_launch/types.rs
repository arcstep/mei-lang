use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_APP_LAUNCH_V1: &str = "mei-app-launch-v1";

/// On-disk launch configuration for a single app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLaunchConfig {
    #[serde(default = "default_schema")]
    pub schema_version: String,
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Explicit `WS-*` or `"current"`.
    #[serde(default = "default_generation")]
    pub generation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_mode_ceiling: Option<String>,
    /// Full or app-scoped runtimePlan object (same shape as deploy.runtimePlan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_plan: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmup: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<Value>,
}

fn default_schema() -> String {
    SCHEMA_APP_LAUNCH_V1.to_string()
}

fn default_generation() -> String {
    "current".to_string()
}

impl AppLaunchConfig {
    pub fn default_for_app(app_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_APP_LAUNCH_V1.to_string(),
            app_id: app_id.to_string(),
            display_name: None,
            generation: "current".to_string(),
            data_mode_ceiling: None,
            runtime_plan: Some(serde_json::json!({
                "defaultMode": "lazy",
                "apps": {}
            })),
            theme: None,
            warmup: None,
            menu: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLaunchSummary {
    pub id: String,
    pub path: String,
    pub revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLaunchDocument {
    pub id: String,
    pub path: String,
    pub revision: String,
    pub config: AppLaunchConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_launch_roundtrip() {
        let cfg = AppLaunchConfig::default_for_app("mini-data");
        let raw = serde_json::to_string(&cfg).expect("ser");
        let back: AppLaunchConfig = serde_json::from_str(&raw).expect("de");
        assert_eq!(back.app_id, "mini-data");
        assert_eq!(back.generation, "current");
        assert_eq!(back.schema_version, SCHEMA_APP_LAUNCH_V1);
    }
}
