//! 统一 `.mei-config.json` 加载：discover / menu / runtime / ops 分层。
//!
//! `.mei` 真源只读；宿主仅通过 ops 白名单对象写回配置，不写 `.mei`。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MEI_CONFIG_FILENAME: &str = ".mei-config.json";
pub const OPS_JOURNAL_REL_PATH: &str = "ops/.mei-ops-journal.json";

/// 可运维对象白名单（宿主写操作仅允许触及这些分类）。
pub const OPS_OBJECT_KINDS: &[&str] = &[
    "theme_ref",
    "source_ref",
    "dataset_source_ref",
    "resource_ref",
    "basemap_ref",
    "mapspec_ref",
    "ops_param_ref",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeiConfig {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default)]
    pub discover: DiscoverConfig,
    #[serde(default)]
    pub menu: Value,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub ops: OpsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoverConfig {
    #[serde(default)]
    pub skip_directories: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub file_cache: FileCacheConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileCacheConfig {
    #[serde(default)]
    pub max_file_mb: Option<usize>,
    #[serde(default)]
    pub max_entries: Option<usize>,
    #[serde(default)]
    pub max_total_mb: Option<usize>,
}

impl FileCacheConfig {
    pub fn to_cache_settings(&self) -> FileCacheSettings {
        FileCacheSettings {
            max_file_bytes: self
                .max_file_mb
                .map(|mb| mb.saturating_mul(1024 * 1024))
                .unwrap_or(10 * 1024 * 1024),
            max_entries: self.max_entries.unwrap_or(100),
            max_total_bytes: self
                .max_total_mb
                .map(|mb| mb.saturating_mul(1024 * 1024))
                .unwrap_or(256 * 1024 * 1024),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FileCacheSettings {
    pub max_file_bytes: usize,
    pub max_entries: usize,
    pub max_total_bytes: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpsConfig {
    #[serde(default)]
    pub themes: BTreeMap<String, Value>,
    #[serde(default)]
    pub sources: BTreeMap<String, OpsSourceEntry>,
    #[serde(default)]
    pub basemaps: BTreeMap<String, OpsBasemapEntry>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsSourceEntry {
    pub kind: String,
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpsBasemapEntry {
    #[serde(default, rename = "tilesBaseUrl")]
    pub tiles_base_url: Option<String>,
    #[serde(default, rename = "tilejsonPath")]
    pub tilejson_path: Option<String>,
    #[serde(default, rename = "layerSpec")]
    pub layer_spec: Option<Value>,
    #[serde(default)]
    pub style: Option<Value>,
}

impl MeiConfig {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read mei config {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse mei config {}", path.display()))
    }

    pub fn load_or_default(path: &Path) -> Self {
        Self::load_from_path(path).unwrap_or_default()
    }

    pub fn discover_skip_directories(&self) -> Vec<String> {
        self.discover
            .skip_directories
            .iter()
            .map(|d| d.trim().trim_matches('/').replace('\\', "/"))
            .filter(|d| !d.is_empty() && !d.contains('/'))
            .collect()
    }
}

/// 应用根优先，其次 workspace segment（`app_root` 的父目录）。
pub fn resolve_mei_config_path(app_root: &Path, source_root: Option<&Path>) -> PathBuf {
    let app_cfg = app_root.join(MEI_CONFIG_FILENAME);
    if app_cfg.is_file() {
        return app_cfg;
    }
    if let Some(segment) = source_root
        .and_then(|root| app_root.strip_prefix(root).ok())
        .and_then(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
    {
        if let Some(root) = source_root {
            let segment_cfg = root.join(segment).join(MEI_CONFIG_FILENAME);
            if segment_cfg.is_file() {
                return segment_cfg;
            }
        }
    }
    if let Some(parent) = app_root.parent() {
        let parent_cfg = parent.join(MEI_CONFIG_FILENAME);
        if parent_cfg.is_file() {
            return parent_cfg;
        }
    }
    app_cfg
}

pub fn load_mei_config_for_app(app_root: &Path, source_root: Option<&Path>) -> MeiConfig {
    let path = resolve_mei_config_path(app_root, source_root);
    MeiConfig::load_or_default(&path)
}

pub fn segment_mei_config_path(segment_root: &Path) -> PathBuf {
    segment_root.join(MEI_CONFIG_FILENAME)
}

pub fn write_mei_config(path: &Path, config: &MeiConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mei config parent dir {}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(config).context("failed to serialize mei config")?;
    fs::write(path, raw).with_context(|| format!("failed to write mei config {}", path.display()))
}

pub fn merge_ops_section(config: &mut MeiConfig, patch: &OpsConfigPatch) -> Result<()> {
    if let Some(themes) = &patch.themes {
        for (key, value) in themes {
            if value.is_null() {
                config.ops.themes.remove(key);
            } else {
                config.ops.themes.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(sources) = &patch.sources {
        for (key, value) in sources {
            if value.is_null() {
                config.ops.sources.remove(key);
            } else {
                let entry: OpsSourceEntry = serde_json::from_value(value.clone())
                    .with_context(|| format!("invalid ops.sources entry `{key}`"))?;
                config.ops.sources.insert(key.clone(), entry);
            }
        }
    }
    if let Some(basemaps) = &patch.basemaps {
        for (key, value) in basemaps {
            if value.is_null() {
                config.ops.basemaps.remove(key);
            } else {
                let entry: OpsBasemapEntry = serde_json::from_value(value.clone())
                    .with_context(|| format!("invalid ops.basemaps entry `{key}`"))?;
                config.ops.basemaps.insert(key.clone(), entry);
            }
        }
    }
    if let Some(params) = &patch.params {
        for (key, value) in params {
            if value.is_null() {
                config.ops.params.remove(key);
            } else {
                config.ops.params.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(())
}

/// 宿主写 ops 时允许的 patch 形状（仅 ops 子树，禁止其它顶层键）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpsConfigPatch {
    #[serde(default)]
    pub themes: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub sources: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub basemaps: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub params: Option<BTreeMap<String, Value>>,
}

impl OpsConfigPatch {
    pub fn is_empty(&self) -> bool {
        self.themes.is_none()
            && self.sources.is_none()
            && self.basemaps.is_none()
            && self.params.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_skip_normalizes_segments() {
        let cfg = MeiConfig {
            discover: DiscoverConfig {
                skip_directories: vec![" /foo/ ".into(), "nested/bad".into(), "ok".into()],
            },
            ..Default::default()
        };
        assert_eq!(cfg.discover_skip_directories(), vec!["foo", "ok"]);
    }
}
