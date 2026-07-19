//! Unified `app.toml` manifest (0540).
//!
//! Runtime loading is **toml-only**. `app.config.json` + `launch.json` remain available
//! only through [`migrate_json_pair_to_toml`] / [`load_app_manifest_from_json_pair`] for
//! one-shot migration tooling.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{
    AppEntryConfig, AppFeaturesConfig, AppPathsConfig, DiscoverConfig, MeiConfig, OpsConfig,
    RuntimeConfig, WorkspaceAuthConfig, APP_CONFIG_FILENAME, APP_TOML_FILENAME,
};

pub const APP_TOML_SCHEMA: &str = "mei-app-v1";
pub const LAUNCH_SCHEMA_V1: &str = "mei-app-launch-v1";

/// In-memory unified app metadata.
#[derive(Debug, Clone, Default)]
pub struct AppManifest {
    pub title: Option<String>,
    pub short_title: Option<String>,
    pub default_stage: Option<String>,
    pub mei: MeiConfig,
    pub app_id: Option<String>,
    pub generation: String,
    pub data_mode_ceiling: Option<String>,
    pub runtime_plan: Option<Value>,
    pub theme: Option<Value>,
    pub warmup: Option<Value>,
    pub launch_menu: Option<Value>,
    /// Source path used for revision hashing (toml or launch.json).
    pub source_path: Option<PathBuf>,
    pub source_raw: Option<String>,
}

impl AppManifest {
    pub fn app_toml_path(app_root: &Path) -> PathBuf {
        app_root.join(APP_TOML_FILENAME)
    }

    pub fn has_app_toml(app_root: &Path) -> bool {
        Self::app_toml_path(app_root).is_file()
    }

    pub fn to_mei_config(&self) -> MeiConfig {
        let mut mei = self.mei.clone();
        mei.apply_profile_runtime_defaults();
        mei
    }

    /// Project launch-shaped JSON for host-core consumers.
    pub fn to_launch_json_value(&self, app_id: &str) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "schemaVersion".to_string(),
            Value::String(LAUNCH_SCHEMA_V1.to_string()),
        );
        map.insert("appId".to_string(), Value::String(app_id.to_string()));
        if let Some(title) = self.title.as_ref().filter(|s| !s.trim().is_empty()) {
            map.insert("displayName".to_string(), Value::String(title.clone()));
        }
        if let Some(short_title) = self.short_title.as_ref().filter(|s| !s.trim().is_empty()) {
            map.insert("shortTitle".to_string(), Value::String(short_title.clone()));
        }
        map.insert(
            "generation".to_string(),
            Value::String(if self.generation.trim().is_empty() {
                "current".to_string()
            } else {
                self.generation.clone()
            }),
        );
        if let Some(ceiling) = &self.data_mode_ceiling {
            map.insert(
                "dataModeCeiling".to_string(),
                Value::String(ceiling.clone()),
            );
        }
        if let Some(plan) = &self.runtime_plan {
            map.insert("runtimePlan".to_string(), plan.clone());
        }
        if let Some(theme) = &self.theme {
            map.insert("theme".to_string(), theme.clone());
        }
        if let Some(warmup) = &self.warmup {
            map.insert("warmup".to_string(), normalize_warmup_for_json(warmup));
        }
        if let Some(menu) = &self.launch_menu {
            map.insert("menu".to_string(), menu.clone());
        } else if !self.mei.menu.is_null() {
            map.insert("menu".to_string(), self.mei.menu.clone());
        }
        Value::Object(map)
    }
}

/// On-disk `app.toml` document (0540 flat layout).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppTomlDocument {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "schemaVersion"
    )]
    pub schema_version: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "shortTitle")]
    pub short_title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "defaultStage"
    )]
    pub default_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "appId")]
    pub app_id: Option<String>,
    #[serde(default)]
    pub entry: AppEntryConfig,
    #[serde(default)]
    pub paths: AppPathsConfig,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub host: Value,
    #[serde(default)]
    pub features: AppFeaturesConfig,
    #[serde(default)]
    pub discover: DiscoverConfig,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub menu: Value,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub ops: OpsConfig,
    #[serde(default, skip_serializing_if = "WorkspaceAuthConfig::is_empty")]
    pub auth: WorkspaceAuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "dataModeCeiling",
        rename = "data_mode_ceiling"
    )]
    pub data_mode_ceiling: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "runtimePlan",
        rename = "runtime_plan"
    )]
    pub runtime_plan: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmup: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "displayName",
        rename = "display_name"
    )]
    pub display_name: Option<String>,
}

impl AppTomlDocument {
    pub fn from_manifest(m: &AppManifest) -> Self {
        let mei = &m.mei;
        Self {
            schema_version: Some(Value::String(APP_TOML_SCHEMA.to_string())),
            title: m.title.clone().or_else(|| m.title.clone()),
            short_title: m.short_title.clone(),
            default_stage: m.default_stage.clone(),
            app_id: m.app_id.clone(),
            entry: mei.entry.clone(),
            paths: mei.paths.clone(),
            host: mei.host.clone(),
            features: mei.features.clone(),
            discover: mei.discover.clone(),
            menu: if let Some(launch_menu) = &m.launch_menu {
                launch_menu.clone()
            } else {
                mei.menu.clone()
            },
            runtime: mei.runtime.clone(),
            ops: mei.ops.clone(),
            auth: mei.auth.clone(),
            generation: Some(if m.generation.trim().is_empty() {
                "current".to_string()
            } else {
                m.generation.clone()
            }),
            data_mode_ceiling: m.data_mode_ceiling.clone(),
            runtime_plan: m.runtime_plan.clone(),
            theme: m.theme.clone(),
            warmup: m.warmup.as_ref().map(normalize_warmup_for_toml),
            display_name: None,
        }
    }

    pub fn into_manifest(self, source_path: PathBuf, source_raw: String) -> AppManifest {
        let title = self
            .title
            .or(self.display_name)
            .filter(|s| !s.trim().is_empty());
        let schema_version = match self.schema_version {
            Some(Value::Number(n)) => n.as_u64().unwrap_or(1) as u32,
            Some(Value::String(_)) | None => 1,
            Some(_) => 1,
        };
        let mei = MeiConfig {
            schema_version,
            entry: self.entry,
            paths: self.paths,
            host: self.host,
            features: self.features,
            discover: self.discover,
            menu: self.menu.clone(),
            runtime: self.runtime,
            ops: self.ops,
            auth: self.auth,
        };
        AppManifest {
            title,
            short_title: self.short_title.filter(|s| !s.trim().is_empty()),
            default_stage: self.default_stage.filter(|s| !s.trim().is_empty()),
            mei,
            app_id: self.app_id,
            generation: self
                .generation
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "current".to_string()),
            data_mode_ceiling: self.data_mode_ceiling,
            runtime_plan: self.runtime_plan,
            theme: self.theme,
            warmup: self.warmup.map(|w| normalize_warmup_for_json(&w)),
            launch_menu: if self.menu.is_null() {
                None
            } else {
                Some(self.menu)
            },
            source_path: Some(source_path),
            source_raw: Some(source_raw),
        }
    }
}

pub fn load_app_manifest(app_root: &Path) -> AppManifest {
    let toml_path = AppManifest::app_toml_path(app_root);
    if toml_path.is_file() {
        return load_app_manifest_from_toml(&toml_path).unwrap_or_else(|_| {
            let mut m = AppManifest {
                default_stage: default_stage_from_registry(app_root),
                title: default_title_from_registry(app_root),
                generation: "current".to_string(),
                source_path: Some(toml_path.clone()),
                ..AppManifest::default()
            };
            m.mei = MeiConfig::default();
            m
        });
    }
    // Hard cut (0121 A6): do not dual-read app.config.json / launch.json.
    AppManifest {
        default_stage: default_stage_from_registry(app_root),
        title: default_title_from_registry(app_root),
        generation: "current".to_string(),
        ..AppManifest::default()
    }
}

pub fn load_app_manifest_from_toml(path: &Path) -> Result<AppManifest, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc: AppTomlDocument = toml::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(doc.into_manifest(path.to_path_buf(), raw))
}

pub fn load_app_manifest_from_json_pair(app_root: &Path) -> AppManifest {
    let config_path = app_root.join(APP_CONFIG_FILENAME);
    let mei = MeiConfig::load_or_default(&config_path);
    let launch_path = app_root.join("launch.json");
    let (
        mut title,
        generation,
        data_mode_ceiling,
        runtime_plan,
        theme,
        warmup,
        launch_menu,
        app_id,
    ) = if launch_path.is_file() {
        match fs::read_to_string(&launch_path) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(v) => (
                    v.get("displayName")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    v.get("generation")
                        .and_then(|x| x.as_str())
                        .unwrap_or("current")
                        .to_string(),
                    v.get("dataModeCeiling")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    v.get("runtimePlan").cloned(),
                    v.get("theme").cloned(),
                    v.get("warmup").cloned(),
                    v.get("menu").cloned(),
                    v.get("appId")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                ),
                Err(_) => (
                    None,
                    "current".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            },
            Err(_) => (
                None,
                "current".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        }
    } else {
        (
            None,
            "current".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
    };

    // Product fields: never scan app.mei skeleton (0120 C4). Fall back to Stage Registry.
    let default_stage = default_stage_from_registry(app_root);
    if title.is_none() {
        title = default_title_from_registry(app_root);
    }

    let source_raw = launch_path
        .is_file()
        .then(|| fs::read_to_string(&launch_path).ok())
        .flatten();

    AppManifest {
        title,
        short_title: None,
        default_stage,
        mei,
        app_id,
        generation,
        data_mode_ceiling,
        runtime_plan,
        theme,
        warmup,
        launch_menu,
        source_path: if launch_path.is_file() {
            Some(launch_path)
        } else if config_path.is_file() {
            Some(config_path)
        } else {
            None
        },
        source_raw,
    }
}

pub fn write_app_toml(app_root: &Path, manifest: &AppManifest) -> Result<(), String> {
    let path = AppManifest::app_toml_path(app_root);
    let mut doc = AppTomlDocument::from_manifest(manifest);
    doc.title = manifest.title.clone();
    let raw = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())
}

/// Convert existing `app.config.json` + `launch.json` into `app.toml` (does not delete JSON).
pub fn migrate_json_pair_to_toml(app_root: &Path) -> Result<(), String> {
    let manifest = load_app_manifest_from_json_pair(app_root);
    write_app_toml(app_root, &manifest)
}

fn normalize_warmup_for_toml(warmup: &Value) -> Value {
    rename_hot_scenes_keys(warmup, "hotScenes", "hot_stages")
}

fn normalize_warmup_for_json(warmup: &Value) -> Value {
    // Consumers still expect hotScenes in JSON launch projection.
    rename_hot_scenes_keys(warmup, "hot_stages", "hotScenes")
}

fn rename_hot_scenes_keys(value: &Value, from: &str, to: &str) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let key = if k == from { to.to_string() } else { k.clone() };
                out.insert(key, rename_hot_scenes_keys(v, from, to));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| rename_hot_scenes_keys(v, from, to))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn default_stage_from_registry(app_root: &Path) -> Option<String> {
    let programs = mei_syntax::discover_stage_programs(app_root);
    if let Some(home) = programs.iter().find(|p| p.stage_id == "home") {
        return Some(home.stage_id.clone());
    }
    if programs.len() == 1 {
        return Some(programs[0].stage_id.clone());
    }
    None
}

fn default_title_from_registry(app_root: &Path) -> Option<String> {
    let programs = mei_syntax::discover_stage_programs(app_root);
    let home = programs
        .iter()
        .find(|p| p.stage_id == "home")
        .or_else(|| (programs.len() == 1).then(|| &programs[0]))?;
    home.title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn toml_wins_over_json_pair() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("app.config.json"),
            r#"{"schemaVersion":1,"entry":{"main":"src/app.mei"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("launch.json"),
            r#"{"schemaVersion":"mei-app-launch-v1","appId":"demo","displayName":"From JSON","generation":"current"}"#,
        )
        .unwrap();
        fs::write(
            root.join("app.toml"),
            r#"
schema_version = "mei-app-v1"
title = "From TOML"
shortTitle = "TOML"
default_stage = "home"
generation = "current"

[entry]
main = "src/app.mei"
"#,
        )
        .unwrap();
        let m = load_app_manifest(root);
        assert_eq!(m.title.as_deref(), Some("From TOML"));
        assert_eq!(m.short_title.as_deref(), Some("TOML"));
        assert_eq!(m.default_stage.as_deref(), Some("home"));
        assert_eq!(m.mei.entry.main, "src/app.mei");
    }

    #[test]
    fn json_pair_is_not_an_app_manifest_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("app.config.json"),
            r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("launch.json"),
            r#"{"schemaVersion":"mei-app-launch-v1","appId":"demo","displayName":"JSON Title","generation":"current","runtimePlan":{"defaultMode":"lazy"}}"#,
        )
        .unwrap();
        let m = load_app_manifest(root);
        assert_eq!(m.title, None);
        assert!(m.mei.entry.main.is_empty());
        assert!(m.runtime_plan.is_none());
    }

    #[test]
    fn warmup_hot_stages_roundtrip_to_json_projection() {
        let mut m = AppManifest::default();
        m.warmup = Some(serde_json::json!({
            "enabled": true,
            "apps": { "demo": { "hot_stages": ["home"] } }
        }));
        let launch = m.to_launch_json_value("demo");
        let hot = launch
            .pointer("/warmup/apps/demo/hotScenes")
            .and_then(|v| v.as_array())
            .expect("hotScenes");
        assert_eq!(hot[0].as_str(), Some("home"));
    }

    #[test]
    fn write_and_reload_app_toml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut m = AppManifest::default();
        m.title = Some("Demo".into());
        m.short_title = Some("DM".into());
        m.default_stage = Some("home".into());
        m.mei.entry.main = "src/app.mei".into();
        m.generation = "current".into();
        write_app_toml(root, &m).expect("write");
        let loaded = load_app_manifest(root);
        assert_eq!(loaded.title.as_deref(), Some("Demo"));
        assert_eq!(loaded.short_title.as_deref(), Some("DM"));
        assert_eq!(loaded.default_stage.as_deref(), Some("home"));
        assert_eq!(loaded.mei.entry.main, "src/app.mei");
    }
}
