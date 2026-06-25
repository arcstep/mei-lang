use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const APP_CONFIG_FILENAME: &str = "app.config.json";
/// App 级配置文件名（v2：`apps/{id}/app.config.json`）。
pub const MEI_CONFIG_FILENAME: &str = APP_CONFIG_FILENAME;
pub const WORKSPACE_CONFIG_FILENAME: &str = "workspace.json";
pub const MEI_WORKSPACE_CONFIG_FILENAME: &str = WORKSPACE_CONFIG_FILENAME;
pub const OPS_JOURNAL_REL_PATH: &str = "ops/.mei-ops-journal.json";
pub const DEFAULT_APPS_REL: &str = "apps";
pub const DEFAULT_APP_SRC_REL: &str = "src";
pub const DEFAULT_TOOLCHAIN_REL: &str = "toolchain";
pub const TOOLCHAIN_STORE_REL: &str = "store";
pub const TOOLCHAIN_ACTIVE_REL: &str = "active";
pub const WORKSPACE_RUNTIME_DIR_REL: &str = "runtime";
pub const DEFAULT_RUNTIME_REL: &str = WORKSPACE_RUNTIME_DIR_REL;
pub const DEFAULT_DEPLOY_REL: &str = "deploy";
pub const APP_BUILD_ACTIVE_REL: &str = "build/active";
pub const APP_BUILD_STORE_REL: &str = "build/store";
pub const APP_VAR_ACTIVE_REL: &str = "var/active";
pub const APP_VAR_STORE_REL: &str = "var/store";
pub const DEPLOY_LINKS_REL: &str = "state/links.json";
pub const BUILD_MANIFEST_FILENAME: &str = "BUILD.json";
pub const PREBUILD_DIR_REL: &str = "prebuild";
pub const PREBUILD_COMPILE_INDEX_REL: &str = "prebuild/compile-index.json";
pub const PREBUILD_LAST_BUILD_SUMMARY_REL: &str = "prebuild/last-build-summary.json";
pub const WORKSPACE_PLATFORM_DIR_REL: &str = "runtime/platform";
pub const WORKSPACE_RUNTIME_LOGS_REL: &str = "runtime/logs";
pub const WORKSPACE_RUNTIME_CACHE_REL: &str = "runtime/cache";
pub const WORKSPACE_LOCAL_DIR_REL: &str = "runtime";
pub const WORKSPACE_HOSTS_DIR_REL: &str = "runtime/hosts";
pub const WORKSPACE_AUTH_DIR_REL: &str = "runtime/hosts";
pub const WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL: &str = "runtime/platform/warmup-manifest.json";
pub const AUTH_JOURNAL_REL_PATH: &str = "runtime/hosts/auth-journal.json";
pub const LEGACY_AUTH_JOURNAL_REL_PATH: &str = ".mei/local/auth/auth-journal.json";
pub const PRE_LOCAL_AUTH_JOURNAL_REL_PATH: &str = "auth/.mei-auth-journal.json";
pub const WORKSPACE_AGENT_LOCAL_DIR_REL: &str = "runtime/agent";
pub const WORKSPACE_AGENT_DB_REL: &str = "runtime/agent/agent.sqlite";
pub const LEGACY_WORKSPACE_AGENT_DB_REL: &str = ".mei/local/agent/agent.sqlite";
pub const WORKSPACE_SNAPSHOT_DIR_REL: &str = "runtime/agent/snapshot";
pub const WORKSPACE_SNAPSHOT_GIT_REL: &str = "runtime/agent/snapshot/git";
pub const LEGACY_WORKSPACE_SNAPSHOT_DIR_REL: &str = ".mei/local/agent/snapshot";
pub const LEGACY_WORKSPACE_SNAPSHOT_GIT_REL: &str = ".mei/local/agent/snapshot/git";
pub const DEFAULT_HOST_STATE_ID: &str = "default";
pub const WORKSPACE_HOST_STATE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_STOCK_COMPONENTS_REL: &str = "stock/components";
pub const DEFAULT_STOCK_TEMPLATES_REL: &str = "stock/templates";
pub const DEFAULT_STOCK_AUTHORING_REL: &str = "stock/authoring";
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
    /// 登录后 `/` 与无 `next` 时的默认应用（须为 discover 到的 app id 或 `discover.appAliases` 别名）。
    #[serde(default, rename = "defaultApp")]
    pub default_app: Option<String>,
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

/// Workspace 级 ops：宿主 shell 主题（与 app `ops.themes` 独立）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceOpsConfig {
    /// 宿主 shell 主题 id，对应 `themes` 表中的条目。
    #[serde(default, rename = "shellTheme")]
    pub shell_theme: Option<String>,
    /// Workspace 级主题表（仅 shell chrome；与 app `ops.themes` 独立）。
    #[serde(default)]
    pub themes: BTreeMap<String, Value>,
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
    /// 宿主 shell 主题与 workspace 级主题表。
    #[serde(default)]
    pub ops: WorkspaceOpsConfig,
    /// 发布 / bundle 运维配置（Git 真源，不含 host-state）。
    #[serde(default)]
    pub deploy: WorkspaceDeployConfig,
    /// 工作区绑定的 mei 工具链版本 pin（84 §3.1）。
    #[serde(default)]
    pub toolchain: WorkspaceToolchainConfig,
    /// 应用 build store 保留策略（84 §4.2）。
    #[serde(default)]
    pub build: WorkspaceBuildConfig,
    /// 工作区 stock 目录、构建树 catalog 与 preview 边界（87 §1）。
    #[serde(default)]
    pub stock: WorkspaceStockConfig,
}

/// 工作区 stock 真源配置：catalog 过滤、preview 边界、未来多 pack 来源。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceStockConfig {
    #[serde(default)]
    pub bootstrap: WorkspaceStockBootstrapConfig,
    #[serde(default)]
    pub catalog: WorkspaceStockCatalogConfig,
    #[serde(default)]
    pub preview: WorkspaceStockPreviewConfig,
    /// 预留：外部 stock pack 来源（git/tar/registry）；本阶段可为空。
    #[serde(default)]
    pub sources: Vec<WorkspaceStockSourceEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceStockBootstrapConfig {
    /// `platform-default` = mei-lang 包内 stock bootstrap。
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceStockCatalogConfig {
    #[serde(default)]
    pub components: WorkspaceStockCatalogKindConfig,
    #[serde(default)]
    pub templates: WorkspaceStockCatalogKindConfig,
    #[serde(default)]
    pub authoring: WorkspaceStockCatalogKindConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStockCatalogKindConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for WorkspaceStockCatalogKindConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStockPreviewConfig {
    #[serde(default = "default_true", rename = "workspaceOnly")]
    pub workspace_only: bool,
    #[serde(default)]
    pub contracts: Option<String>,
}

impl Default for WorkspaceStockPreviewConfig {
    fn default() -> Self {
        Self {
            workspace_only: true,
            contracts: Some(format!(
                "{DEFAULT_STOCK_AUTHORING_REL}/component-contracts.json"
            )),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceStockSourceEntry {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

fn default_true() -> bool {
    true
}

impl WorkspaceConfig {
    pub fn preview_workspace_only(&self) -> bool {
        self.stock.preview.workspace_only
    }

    pub fn stock_contracts_rel(&self) -> Option<&str> {
        self.stock
            .preview
            .contracts
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceToolchainConfig {
    #[serde(default)]
    pub pin: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceBuildConfig {
    #[serde(default, rename = "retainBuildGenerations")]
    pub retain_build_generations: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDeployAccessEntry {
    #[serde(default, rename = "defaultApp")]
    pub default_app: Option<String>,
    #[serde(default, rename = "defaultScene")]
    pub default_scene: Option<String>,
    #[serde(default, rename = "targetFile")]
    pub target_file: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDeployReachabilityGate {
    #[serde(default, rename = "requireMcgAssemblyReady")]
    pub require_mcg_assembly_ready: Option<bool>,
    #[serde(default, rename = "requireMrgCriticalReady")]
    pub require_mrg_critical_ready: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDeployConfig {
    #[serde(default, rename = "bundleIdFormat")]
    pub bundle_id_format: Option<String>,
    #[serde(default, rename = "retainBundles")]
    pub retain_bundles: Option<u32>,
    #[serde(default, rename = "pinnedBundles")]
    pub pinned_bundles: Vec<String>,
    #[serde(default, rename = "accessEntry")]
    pub access_entry: WorkspaceDeployAccessEntry,
    #[serde(default, rename = "candidatePort")]
    pub candidate_port: Option<u16>,
    #[serde(default, rename = "productionPort")]
    pub production_port: Option<u16>,
    #[serde(default, rename = "promotePolicy")]
    pub promote_policy: Option<String>,
    #[serde(default, rename = "reachabilityGate")]
    pub reachability_gate: WorkspaceDeployReachabilityGate,
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
    /// Optional warmup tier. `critical` joins hot phase; `deferred`/`heavy` stays in background.
    #[serde(default)]
    pub priority: Option<String>,
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
    #[serde(default)]
    pub priority: Option<String>,
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
    #[serde(default, rename = "accessAiExternal")]
    pub access_ai_external: Option<AccessAiExternalConfig>,
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
    #[serde(default = "default_client_query_cache_max_entries", rename = "maxEntries")]
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
