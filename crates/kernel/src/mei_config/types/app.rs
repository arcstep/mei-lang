use super::workspace::WorkspaceAuthConfig;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::paths::DEFAULT_APP_ENTRY_MAIN;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// app 根目录 `.mei-config.json`：入口、路径、宿主能力与 ops。
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
    #[serde(default)]
    pub presentation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessAiExternalConfig {
    pub url: String,
    pub image: String,
    #[serde(default, rename = "openInNewTab")]
    pub open_in_new_tab: Option<bool>,
    #[serde(default)]
    pub label: Option<String>,
}

impl AccessAiExternalConfig {
    pub fn is_configured(&self) -> bool {
        !self.url.trim().is_empty() && !self.image.trim().is_empty()
    }

    pub fn open_in_new_tab(&self) -> bool {
        self.open_in_new_tab.unwrap_or(true)
    }

    pub fn label_or_default(&self) -> &str {
        self.label
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("打开 AI 助手")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppFeaturesConfig {
    #[serde(default, rename = "aiChat")]
    pub ai_chat: Option<bool>,
    #[serde(default, rename = "sceneBundle")]
    pub scene_bundle: Option<bool>,
    /// Access 内置 Copilot FAB（`#access-chat-fab`）。`Some(false)` 关闭；缺省/true 开启。
    /// 与 `accessAiExternal` 无关（外链 FAB 仍由后者单独配置）。
    #[serde(default, rename = "copilotFab")]
    pub copilot_fab: Option<bool>,
    #[serde(default, rename = "accessAiExternal")]
    pub access_ai_external: Option<AccessAiExternalConfig>,
}

impl AppFeaturesConfig {
    /// 内置 Copilot FAB 是否启用；仅当显式 `copilotFab = false` 时关闭。
    pub fn copilot_fab_enabled(&self) -> bool {
        self.copilot_fab != Some(false)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspacePathsConfig {
    /// 应用目录根（相对 workspace 根），默认 `apps`。
    #[serde(default)]
    pub apps: Option<String>,
    /// 组件库根（相对 workspace 根），默认 `stock/components`。
    #[serde(default)]
    pub components: Option<String>,
    /// 模板库根，默认 `stock/templates`。
    #[serde(default)]
    pub templates: Option<String>,
    /// authoring helper 根，默认 `stock/authoring`。
    #[serde(default)]
    pub authoring: Option<String>,
    /// 工具链根，默认 `toolchain`。
    #[serde(default)]
    pub toolchain: Option<String>,
    /// 工作区 runtime 根，默认 `runtime`。
    #[serde(default)]
    pub runtime: Option<String>,
    /// 发布脚本根，默认 `deploy`。
    #[serde(default)]
    pub deploy: Option<String>,
    /// 共享 stock 根，默认 `stock`（components/templates 的父目录，实现期可选）。
    #[serde(default)]
    pub stock: Option<String>,
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
    #[serde(default, rename = "cacheGeneration")]
    pub cache_generation: CacheGenerationConfig,
    #[serde(default, rename = "clientQueryCache")]
    pub client_query_cache: ClientQueryCacheConfig,
    #[serde(default, rename = "serverEvalCache")]
    pub server_eval_cache: ServerEvalCacheConfig,
    #[serde(default, rename = "memoryWarmup")]
    pub memory_warmup: Option<MemoryWarmupConfig>,
    #[serde(default, rename = "clientBootstrap")]
    pub client_bootstrap: Option<ClientBootstrapConfig>,
    #[serde(default, rename = "smartWarmup")]
    pub smart_warmup: Option<SmartWarmupConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheGenerationConfig {
    #[serde(default)]
    pub sources: CacheGenerationSourcesConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheGenerationSourcesConfig {
    #[serde(default)]
    pub file: CacheGenerationSourceModeConfig,
    #[serde(default)]
    pub database: CacheGenerationSourceModeConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheGenerationSourceModeConfig {
    #[serde(default)]
    pub mode: String,
    #[serde(default, rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientQueryCacheConfig {
    #[serde(default = "default_client_query_cache_persist")]
    pub persist: String,
    #[serde(default = "default_client_query_cache_ttl_ms", rename = "ttlMs")]
    pub ttl_ms: u64,
    #[serde(
        default = "default_client_query_cache_max_entries",
        rename = "maxEntries"
    )]
    pub max_entries: usize,
}

fn default_client_query_cache_persist() -> String {
    "sessionStorage".to_string()
}

fn default_client_query_cache_ttl_ms() -> u64 {
    300_000
}

fn default_client_query_cache_max_entries() -> usize {
    512
}

impl Default for ClientQueryCacheConfig {
    fn default() -> Self {
        Self {
            persist: default_client_query_cache_persist(),
            ttl_ms: default_client_query_cache_ttl_ms(),
            max_entries: default_client_query_cache_max_entries(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvalCacheConfig {
    #[serde(default = "default_server_eval_cache_ttl_ms", rename = "ttlMs")]
    pub ttl_ms: u64,
}

fn default_server_eval_cache_ttl_ms() -> u64 {
    300_000
}

impl Default for ServerEvalCacheConfig {
    fn default() -> Self {
        Self {
            ttl_ms: default_server_eval_cache_ttl_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWarmupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_memory_pin_max_slots", rename = "maxPinnedSlots")]
    pub max_pinned_slots: usize,
    #[serde(default = "default_memory_pin_max_mb", rename = "maxPinnedMb")]
    pub max_pinned_mb: usize,
    /// When false (default), `::__scalar_rowset__` payloads are not pinned into L1.
    /// Disk packs remain complete for Pack-First; rowsets load on demand.
    #[serde(default = "default_memory_pin_rowsets", rename = "pinRowsets")]
    pub pin_rowsets: bool,
    /// Drop individual metric values larger than this from L1 pin projection.
    #[serde(
        default = "default_memory_max_pinned_value_bytes",
        rename = "maxPinnedValueBytes"
    )]
    pub max_pinned_value_bytes: usize,
}

fn default_memory_pin_max_slots() -> usize {
    64
}

fn default_memory_pin_max_mb() -> usize {
    128
}

fn default_memory_pin_rowsets() -> bool {
    false
}

fn default_memory_max_pinned_value_bytes() -> usize {
    256 * 1024
}

impl Default for MemoryWarmupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_pinned_slots: default_memory_pin_max_slots(),
            max_pinned_mb: default_memory_pin_max_mb(),
            pin_rowsets: default_memory_pin_rowsets(),
            max_pinned_value_bytes: default_memory_max_pinned_value_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBootstrapConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(
        default = "default_client_bootstrap_max_metrics",
        rename = "maxMetricsPerScope"
    )]
    pub max_metrics_per_scope: usize,
    #[serde(default, rename = "neighborHops")]
    pub neighbor_hops: usize,
    #[serde(
        default = "default_client_bootstrap_max_neighbor_scopes",
        rename = "maxNeighborScopes"
    )]
    pub max_neighbor_scopes: usize,
    /// `inline` embeds full JSON in HTML; `revision_only` exposes meta + artifact URL only.
    #[serde(default = "default_client_bootstrap_embed_mode", rename = "embedMode")]
    pub embed_mode: String,
}

fn default_client_bootstrap_embed_mode() -> String {
    "revision_only".to_string()
}

fn default_client_bootstrap_max_metrics() -> usize {
    256
}

fn default_client_bootstrap_max_neighbor_scopes() -> usize {
    4
}

impl Default for ClientBootstrapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scopes: vec!["home".to_string()],
            max_metrics_per_scope: default_client_bootstrap_max_metrics(),
            neighbor_hops: 0,
            max_neighbor_scopes: default_client_bootstrap_max_neighbor_scopes(),
            embed_mode: default_client_bootstrap_embed_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWarmupConfig {
    #[serde(default)]
    pub enabled: bool,
}

impl Default for SmartWarmupConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl CacheGenerationSourcesConfig {
    pub fn file_mode(&self) -> &str {
        let trimmed = self.file.mode.trim();
        if trimmed.is_empty() {
            "manual_reload"
        } else {
            trimmed
        }
    }

    pub fn database_ttl_ms(&self) -> u64 {
        self.database
            .ttl_ms
            .unwrap_or(crate::cache_generation::DEFAULT_DATABASE_TTL_MS)
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsConfig {
    #[serde(default)]
    pub themes: BTreeMap<String, Value>,
    #[serde(default)]
    pub sources: BTreeMap<String, OpsSourceEntry>,
    #[serde(default)]
    pub basemaps: BTreeMap<String, OpsBasemapEntry>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// P1–P2: layout budget overlay (see docs/mei-lang-v2/03-ui/0325-layout-budget-contract.md §8).
    /// When true, T1 cockpit sections must use Fill-down (`__mei_layout_fill`).
    /// Defaults to true (Phase 3 layout purge).
    #[serde(default = "default_strict_fill_down", rename = "strictFillDown")]
    pub strict_fill_down: bool,
    /// T2 pages remain under Fill-down by default. Content-driven T2 pages must
    /// opt out explicitly with `ops.fillDown = false`.
    #[serde(default = "default_fill_down", rename = "fillDown")]
    pub fill_down: bool,
    /// App-defined Admin/config namespaces such as `ops.organization`.
    #[serde(default, flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl Default for OpsConfig {
    fn default() -> Self {
        Self {
            themes: BTreeMap::new(),
            sources: BTreeMap::new(),
            basemaps: BTreeMap::new(),
            params: BTreeMap::new(),
            strict_fill_down: true,
            fill_down: true,
            extensions: BTreeMap::new(),
        }
    }
}

fn default_strict_fill_down() -> bool {
    true
}

fn default_fill_down() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsSourceEntry {
    pub kind: String,
    /// File-backed sources use a relative path; postgres/db may leave this empty
    /// and set `connection` + `query`/`table` instead.
    #[serde(default)]
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

    /// Cockpit 应用模板：未显式配置时默认开启 1-hop 邻域 Pack 产能。
    pub fn apply_profile_runtime_defaults(&mut self) {
        if !self.ops.themes.contains_key("cockpit") {
            return;
        }
        let bootstrap = self
            .runtime
            .client_bootstrap
            .get_or_insert_with(ClientBootstrapConfig::default);
        if bootstrap.neighbor_hops == 0 {
            bootstrap.neighbor_hops = 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MeiConfig;

    #[test]
    fn fill_down_defaults_strict_and_allows_explicit_t2_opt_out() {
        let default_config: MeiConfig =
            serde_json::from_str(r#"{"schemaVersion":1}"#).expect("default config");
        assert!(default_config.ops.fill_down);

        let opted_out: MeiConfig =
            serde_json::from_str(r#"{"ops":{"fillDown":false}}"#).expect("opt-out config");
        assert!(!opted_out.ops.fill_down);
    }
}
