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
    if path.is_file() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read app config {}", path.display()))?;
        return serde_json::from_str(&raw)
            .with_context(|| format!("parse app config {}", path.display()));
    }
    // Wave D: toml-only apps — project `app.toml` / MeiConfig into the legacy
    // AppConfig shape so metric hydrate, data snapshots, and other host paths
    // still see `ops.sources` after `app.config.json` is removed.
    Ok(AppConfig::from_mei_config(
        &mei_lang_kernel::load_mei_config_for_app(app_root, None),
    ))
}

impl AppConfig {
    pub fn from_mei_config(mei: &mei_lang_kernel::MeiConfig) -> Self {
        let mut sources = std::collections::BTreeMap::new();
        for (key, entry) in &mei.ops.sources {
            if let Ok(value) = serde_json::to_value(entry) {
                sources.insert(key.clone(), value);
            }
        }
        Self {
            schema_version: mei.schema_version,
            default_launch: None,
            ops: OpsSection {
                sources,
                themes: mei.ops.themes.clone(),
                params: mei.ops.params.clone(),
            },
            runtime: RuntimeSection::default(),
        }
    }
}
