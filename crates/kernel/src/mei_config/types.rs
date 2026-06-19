use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MEI_CONFIG_FILENAME: &str = ".mei-config.json";
pub const MEI_WORKSPACE_CONFIG_FILENAME: &str = ".mei-workspace.json";
pub const OPS_JOURNAL_REL_PATH: &str = "ops/.mei-ops-journal.json";
pub const WORKSPACE_LOCAL_DIR_REL: &str = ".mei/local";
pub const WORKSPACE_HOSTS_DIR_REL: &str = ".mei/local/hosts";
pub const WORKSPACE_AUTH_DIR_REL: &str = ".mei/local/auth";
pub const WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL: &str = ".mei/runtime/warmup-manifest.json";
pub const AUTH_JOURNAL_REL_PATH: &str = ".mei/local/auth/auth-journal.json";
pub const LEGACY_AUTH_JOURNAL_REL_PATH: &str = ".mei/auth/auth-journal.json";
pub const PRE_LOCAL_AUTH_JOURNAL_REL_PATH: &str = "auth/.mei-auth-journal.json";
pub const WORKSPACE_AGENT_LOCAL_DIR_REL: &str = ".mei/local/agent";
pub const WORKSPACE_AGENT_DB_REL: &str = ".mei/local/agent/agent.sqlite";
pub const LEGACY_WORKSPACE_AGENT_DB_REL: &str = ".mei/agent.sqlite";
pub const WORKSPACE_SNAPSHOT_DIR_REL: &str = ".mei/local/agent/snapshot";
pub const WORKSPACE_SNAPSHOT_GIT_REL: &str = ".mei/local/agent/snapshot/git";
pub const LEGACY_WORKSPACE_SNAPSHOT_DIR_REL: &str = ".mei/snapshot";
pub const LEGACY_WORKSPACE_SNAPSHOT_GIT_REL: &str = ".mei/snapshot/git";
pub const DEFAULT_HOST_STATE_ID: &str = "default";
pub const WORKSPACE_HOST_STATE_SCHEMA_VERSION: u32 = 1;
/// 可 Git 跟踪的物化组件库（与运行时 `.mei/` 分离）。
pub const DEFAULT_STOCK_COMPONENTS_REL: &str = ".stock/components";
/// 可 Git 跟踪的物化模板库。
pub const DEFAULT_STOCK_TEMPLATES_REL: &str = ".stock/templates";
/// 可 Git 跟踪的 workspace-local authoring helper（`.star`）目录。
pub const DEFAULT_STOCK_AUTHORING_REL: &str = ".stock/authoring";
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

/// 工作区 profile 元数据（`workspaces/ws-*` 根目录 `.mei-workspace.json`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceProfile {
    /// 启动时 `--workspace` 使用的短名，如 `ws-spbjw`。
    pub id: Option<String>,
    pub label: Option<String>,
    #[serde(default, rename = "deployHost")]
    pub deploy_host: Option<String>,
}

/// 工作区合规展示信息（登录页与底栏备案号等）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceComplianceConfig {
    /// ICP 备案号，如「渝ICP备xxxxxxxx号」。
    #[serde(default, rename = "icpRecord")]
    pub icp_record: Option<String>,
    /// 公安备案号。
    #[serde(default, rename = "psbRecord")]
    pub psb_record: Option<String>,
    /// 版权或运营主体说明。
    #[serde(default)]
    pub copyright: Option<String>,
}

impl WorkspaceComplianceConfig {
    pub fn icp_record_trimmed(&self) -> Option<&str> {
        self.icp_record
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn psb_record_trimmed(&self) -> Option<&str> {
        self.psb_record
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn copyright_trimmed(&self) -> Option<&str> {
        self.copyright
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// workspace / segment 级配置：发现规则、默认菜单与运行时回退。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default)]
    pub workspace: WorkspaceProfile,
    #[serde(default)]
    pub paths: WorkspacePathsConfig,
    #[serde(default)]
    pub discover: DiscoverConfig,
    #[serde(default)]
    pub menu: Value,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub warmup: WorkspaceWarmupConfig,
    /// 登录页与底栏展示的备案号、版权等合规信息。
    #[serde(default)]
    pub compliance: WorkspaceComplianceConfig,
    /// 工作区级宿主认证配置（用户清单、JWT、登录加密密钥）。
    #[serde(default, skip_serializing_if = "WorkspaceAuthConfig::is_empty")]
    pub auth: WorkspaceAuthConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceWarmupConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub apps: BTreeMap<String, WorkspaceWarmupAppConfig>,
}

impl WorkspaceWarmupConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceWarmupAppConfig {
    #[serde(default, rename = "hotScenes")]
    pub hot_scenes: Vec<String>,
    /// Manage / build preview locators (e.g. `main.mei`). Empty → app entry main.
    #[serde(default)]
    pub focuses: Vec<String>,
    #[serde(default)]
    pub datasets: Vec<WorkspaceWarmupDatasetConfig>,
    /// App-relative xlsx paths to preload into L3 table snapshot cache.
    #[serde(default, rename = "xlsxSources")]
    pub xlsx_sources: Vec<WorkspaceWarmupXlsxConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWarmupXlsxConfig {
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default, rename = "headerRow")]
    pub header_row: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWarmupDatasetConfig {
    #[serde(default, rename = "sceneId")]
    pub scene_id: Option<String>,
    /// Compile focus for dataset warmup; empty → app default focus (entry main).
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default, rename = "datasetId")]
    pub dataset_id: String,
    #[serde(default, rename = "metricId")]
    pub metric_id: Option<String>,
    #[serde(default, rename = "metricIds")]
    pub metric_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeWarmupManifest {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub apps: Vec<RuntimeWarmupApp>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeWarmupApp {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(default, rename = "defaultScene")]
    pub default_scene: Option<String>,
    #[serde(default, rename = "hotScenes")]
    pub hot_scenes: Vec<String>,
    #[serde(default)]
    pub scenes: Vec<String>,
    #[serde(default)]
    pub focuses: Vec<String>,
    #[serde(default)]
    pub datasets: Vec<RuntimeWarmupDatasetRequest>,
    #[serde(default, rename = "xlsxSources")]
    pub xlsx_sources: Vec<RuntimeWarmupXlsxSource>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWarmupXlsxSource {
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default, rename = "headerRow")]
    pub header_row: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWarmupDatasetRequest {
    #[serde(default, rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(rename = "datasetId")]
    pub dataset_id: String,
    #[serde(default, rename = "metricId")]
    pub metric_id: Option<String>,
    #[serde(default, rename = "metricIds")]
    pub metric_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceAuthConfig {
    #[serde(default, rename = "jwtSecret")]
    pub jwt_secret: Option<String>,
    #[serde(default, rename = "jwtTtlSeconds")]
    pub jwt_ttl_seconds: Option<u64>,
    #[serde(default, rename = "cookieName")]
    pub cookie_name: Option<String>,
    #[serde(default)]
    pub users: Vec<AuthUserConfig>,
    #[serde(default, rename = "keyPair")]
    pub key_pair: AuthKeyPairConfig,
}

impl WorkspaceAuthConfig {
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
            && self
                .jwt_secret
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            && self
                .cookie_name
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            && self.jwt_ttl_seconds.is_none()
            && self.key_pair.public_key_pem.trim().is_empty()
            && self.key_pair.private_key_pem.trim().is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceHostState {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default, rename = "hostId")]
    pub host_id: Option<String>,
    #[serde(default, skip_serializing_if = "WorkspaceAuthConfig::is_empty")]
    pub auth: WorkspaceAuthConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthKeyPairConfig {
    #[serde(default, rename = "publicKeyPem")]
    pub public_key_pem: String,
    #[serde(default, rename = "privateKeyPem")]
    pub private_key_pem: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthUserConfig {
    pub username: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default, rename = "passwordHash")]
    pub password_hash: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, rename = "appAllowlist")]
    pub app_allowlist: Vec<String>,
    /// 显式禁止访问的工作区 mei 应用 id；空 `appAllowlist` 时默认允许除 denylist 外全部应用。
    #[serde(default, rename = "appDenylist")]
    pub app_denylist: Vec<String>,
    #[serde(default, rename = "sceneAllowlist")]
    pub scene_allowlist: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub disabled: bool,
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
    /// 兼容一次误将工作区 `auth` 写入 `.mei-config.json` 的迁移窗口；应用运行时不应依赖该字段。
    #[serde(default, skip_serializing_if = "WorkspaceAuthConfig::is_empty")]
    pub auth: WorkspaceAuthConfig,
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
    #[serde(default, rename = "sceneBundle")]
    pub scene_bundle: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspacePathsConfig {
    /// 组件库根（相对 profile 根或绝对路径）；未设且未物化时回退 `mei-lang/stock/components`。
    #[serde(default)]
    pub components: Option<String>,
    /// 模板库根；未设且未物化时回退 `mei-lang/stock/templates`。
    #[serde(default)]
    pub templates: Option<String>,
    /// workspace-local authoring helper 根（相对 segment 根）；加载其中全部 `.star` 并注入求值 prelude。
    #[serde(default)]
    pub authoring: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoverConfig {
    #[serde(default)]
    pub skip_directories: Vec<String>,
    /// 已迁至 `paths.components`；仅兼容旧配置。
    #[serde(default, rename = "componentsRoot")]
    pub components_root: Option<String>,
    /// URL/CLI 旧应用 id → 目录名，如 `spbjw` / `xzjd` → `zhifa`。
    #[serde(default, rename = "appAliases")]
    pub app_aliases: BTreeMap<String, String>,
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

impl WorkspaceHostState {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read workspace host state {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse workspace host state {}", path.display()))
    }

    pub fn load_or_default(path: &Path) -> Self {
        Self::load_from_path(path).unwrap_or_default()
    }
}
