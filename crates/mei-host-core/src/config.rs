use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

/// Parsed subset of `app.config.json` needed by host-shell and plug-ds.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    #[serde(rename = "schemaVersion", default)]
    pub schema_version: u32,
    /// DEPRECATED (Phase 8.5): ignored — each app has a single `launch.json`.
    #[serde(default, rename = "defaultLaunch")]
    pub default_launch: Option<String>,
    #[serde(default)]
    pub ops: OpsSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimeSection {
    #[serde(default)]
    pub plugs: PlugsSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlugsSection {
    #[serde(default)]
    pub ds: Option<PlugEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlugEndpoint {
    pub endpoint: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpsSection {
    #[serde(default)]
    pub sources: std::collections::BTreeMap<String, Value>,
    #[serde(default)]
    pub themes: std::collections::BTreeMap<String, Value>,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct WarmupPolicyRef {
    pub scope_key: String,
    pub assembly_key: String,
    pub priority: String,
}

pub fn load_app_config(app_root: &Path) -> Result<AppConfig> {
    let path = app_root.join("app.config.json");
    if !path.is_file() {
        return Ok(AppConfig::default());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("read app config {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse app config {}", path.display()))
}
