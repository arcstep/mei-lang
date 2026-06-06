//! App 级 `.mei-config.json` 与 workspace 级 `.mei-workspace.json` 分层加载。
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
pub const MEI_WORKSPACE_CONFIG_FILENAME: &str = ".mei-workspace.json";
pub const OPS_JOURNAL_REL_PATH: &str = "ops/.mei-ops-journal.json";
pub const DEFAULT_APP_ENTRY_MAIN: &str = "main.mei";

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

/// workspace / segment 级配置：发现规则、默认菜单与运行时回退。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default)]
    pub discover: DiscoverConfig,
    #[serde(default)]
    pub menu: Value,
    #[serde(default)]
    pub runtime: RuntimeConfig,
}

/// app 根目录 `.mei-config.json`：入口、路径、宿主能力与 ops。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeiConfig {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default)]
    pub entry: AppEntryConfig,
    #[serde(default)]
    pub paths: AppPathsConfig,
    #[serde(default)]
    pub host: Value,
    #[serde(default)]
    pub features: AppFeaturesConfig,
    /// 已迁移至 `.mei-workspace.json`；反序列化保留以兼容旧文件，运行时忽略。
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
pub struct AppEntryConfig {
    #[serde(default)]
    pub main: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppPathsConfig {
    #[serde(default)]
    pub upload: Option<String>,
    #[serde(default)]
    pub prototype: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppFeaturesConfig {
    #[serde(default, rename = "aiChat")]
    pub ai_chat: Option<bool>,
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

impl AppEntryConfig {
    pub fn main_rel(&self) -> String {
        let trimmed = self.main.trim().trim_matches('/');
        if trimmed.is_empty() {
            DEFAULT_APP_ENTRY_MAIN.to_string()
        } else {
            trimmed.replace('\\', "/")
        }
    }
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

    pub fn has_legacy_workspace_fields(&self) -> bool {
        !self.discover.skip_directories.is_empty()
            || !self.menu.is_null()
            || self.runtime.file_cache.max_file_mb.is_some()
            || self.runtime.file_cache.max_entries.is_some()
            || self.runtime.file_cache.max_total_mb.is_some()
    }
}

impl WorkspaceConfig {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read workspace config {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse workspace config {}", path.display()))
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

pub fn is_app_config_root(dir: &Path) -> bool {
    dir.join(MEI_CONFIG_FILENAME).is_file()
}

pub fn app_mei_config_path(app_root: &Path) -> PathBuf {
    app_root.join(MEI_CONFIG_FILENAME)
}

pub fn workspace_config_path(segment_root: &Path) -> PathBuf {
    segment_root.join(MEI_WORKSPACE_CONFIG_FILENAME)
}

/// 仅认 app 根目录的 `.mei-config.json`，不再向上/向 segment 回退。
pub fn resolve_mei_config_path(app_root: &Path, _source_root: Option<&Path>) -> PathBuf {
    app_mei_config_path(app_root)
}

pub fn load_mei_config_for_app(app_root: &Path, source_root: Option<&Path>) -> MeiConfig {
    let path = resolve_mei_config_path(app_root, source_root);
    MeiConfig::load_or_default(&path)
}

/// 迁移窗口：优先 `.mei-workspace.json`，否则回退读取 segment 级旧 `.mei-config.json`。
pub fn load_workspace_config(segment_root: &Path) -> WorkspaceConfig {
    let modern = workspace_config_path(segment_root);
    if modern.is_file() {
        return WorkspaceConfig::load_or_default(&modern);
    }
    let legacy = segment_root.join(MEI_CONFIG_FILENAME);
    if legacy.is_file() {
        let legacy_app = MeiConfig::load_or_default(&legacy);
        return WorkspaceConfig {
            schema_version: legacy_app.schema_version,
            discover: legacy_app.discover,
            menu: legacy_app.menu,
            runtime: legacy_app.runtime,
        };
    }
    WorkspaceConfig::default()
}

pub fn resolve_app_entry_main(app_root: &Path) -> String {
    let path = app_mei_config_path(app_root);
    if path.is_file() {
        MeiConfig::load_or_default(&path).entry.main_rel()
    } else {
        DEFAULT_APP_ENTRY_MAIN.to_string()
    }
}

pub fn resolve_app_main_path(app_root: &Path) -> PathBuf {
    app_root.join(resolve_app_entry_main(app_root))
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
    fn workspace_discover_skip_normalizes_segments() {
        let cfg = WorkspaceConfig {
            discover: DiscoverConfig {
                skip_directories: vec![" /foo/ ".into(), "nested/bad".into(), "ok".into()],
            },
            ..Default::default()
        };
        assert_eq!(cfg.discover_skip_directories(), vec!["foo", "ok"]);
    }

    #[test]
    fn entry_main_defaults_to_main_mei() {
        let entry = AppEntryConfig::default();
        assert_eq!(entry.main_rel(), "main.mei");
        let entry = AppEntryConfig {
            main: " scenes/home.mei ".into(),
        };
        assert_eq!(entry.main_rel(), "scenes/home.mei");
    }
}
