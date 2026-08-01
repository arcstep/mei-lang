use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::app::{DiscoverConfig, RuntimeConfig, WorkspacePathsConfig};
use super::paths::DEFAULT_STOCK_AUTHORING_REL;

/// 工作区顶栏品牌（`workspace.json#workspace.brand`；见 docs 0544）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceBrandConfig {
    /// 顶栏品牌文案；缺省为 MeiLang。
    #[serde(default)]
    pub title: Option<String>,
    /// 工作区内相对路径（建议 `assets/...`）；缺省为 Host favicon。
    #[serde(default)]
    pub logo: Option<String>,
}

impl WorkspaceBrandConfig {
    pub fn title_trimmed(&self) -> Option<&str> {
        self.title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn logo_trimmed(&self) -> Option<&str> {
        self.logo
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// 工作区 profile 元数据（`ws-*` profile 根目录 `.mei-workspace.json`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceProfile {
    /// 启动时 `--workspace` 使用的短名，如 `ws-spbjw`。
    pub id: Option<String>,
    pub label: Option<String>,
    /// 顶栏 WorkspaceBrand；与 `label`（面包屑/合规）分离。
    #[serde(default)]
    pub brand: WorkspaceBrandConfig,
    #[serde(default, rename = "deployHost")]
    pub deploy_host: Option<String>,
    /// 本机 Host 监听端口（`start-host` / `serve`；可被 `--port` / `MEI_PORT` 覆盖）。
    #[serde(default)]
    pub port: Option<u16>,
    /// 本机 Host 绑定地址（默认 `127.0.0.1`；可被 `--host` / `MEI_SERVE_HOST` 覆盖）。
    /// 与 `deployHost`（发布目标 id）无关。
    #[serde(default, rename = "listenHost")]
    pub listen_host: Option<String>,
    /// 登录后 `/` 与无 `next` 时的默认应用（须为 discover 到的 app id 或 `discover.appAliases` 别名）。
    #[serde(default, rename = "defaultApp")]
    pub default_app: Option<String>,
    /// 工作区对外发布版本（客户可读，如 `20260228`）；与 mei-lang toolchain 组合成 env 目录名。
    #[serde(default)]
    pub version: Option<String>,
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

impl WorkspaceProfile {
    pub fn version_trimmed(&self) -> Option<&str> {
        self.version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// Workspace 级 ops：shell chrome 与 scene 色板库分轨。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceOpsConfig {
    /// 宿主 shell 主题 id，对应 `themes` 表中的条目。
    #[serde(default, rename = "shellTheme")]
    pub shell_theme: Option<String>,
    /// Workspace 级主题表（仅 shell chrome；与 scene 色板库独立）。
    #[serde(default)]
    pub themes: BTreeMap<String, Value>,
    /// Scene 色板库（tokens.color / gradient 等；不含应用字号档位）。
    #[serde(default, rename = "sceneThemes")]
    pub scene_themes: BTreeMap<String, Value>,
    /// 默认 scene 色板 id（当应用未设置 theme_selection 时）。
    #[serde(default, rename = "sceneThemeDefault")]
    pub scene_theme_default: Option<String>,
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
    /// 隐藏 Build-only 应用：统一 stock 组件/模板预览 compile/prebuild 管线。
    #[serde(default, rename = "catalogApp")]
    pub catalog_app: WorkspaceStockCatalogAppConfig,
    /// 预留：外部 stock pack 来源（git/tar/registry）；本阶段可为空。
    #[serde(default)]
    pub sources: Vec<WorkspaceStockSourceEntry>,
}

pub const DEFAULT_STOCK_CATALOG_APP_ID: &str = "_stock-catalog";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStockCatalogAppConfig {
    #[serde(default = "default_stock_catalog_app_id")]
    pub id: String,
    #[serde(default = "default_stock_catalog_app_title")]
    pub title: String,
    #[serde(default = "default_true", rename = "buildOnly")]
    pub build_only: bool,
}

impl Default for WorkspaceStockCatalogAppConfig {
    fn default() -> Self {
        Self {
            id: default_stock_catalog_app_id(),
            title: default_stock_catalog_app_title(),
            build_only: true,
        }
    }
}

fn default_stock_catalog_app_id() -> String {
    DEFAULT_STOCK_CATALOG_APP_ID.to_string()
}

fn default_stock_catalog_app_title() -> String {
    "Stock Catalog".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStockBootstrapConfig {
    /// `platform-default` = mei-lang 包内 stock bootstrap。
    #[serde(default)]
    pub source: Option<String>,
    /// When true (default), `ensure_workspace_stock_materialized` keeps copying/refreshing
    /// missing or older files from the platform package. Set `false` for workspace-owned
    /// stock (e.g. after moving unused examples/previews into `stock/legacy/`); then
    /// startup/prebuild will not restore deleted files. Explicit `workspace stock sync`
    /// still force-copies.
    #[serde(default = "default_true")]
    pub refresh: bool,
}

impl Default for WorkspaceStockBootstrapConfig {
    fn default() -> Self {
        Self {
            source: None,
            refresh: true,
        }
    }
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceToolchainConfig {
    #[serde(default)]
    pub pin: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceBuildGenerationConfig {
    /// `auto` (default) uses today's date at prebuild; `manual` uses `date`.
    #[serde(default, rename = "dateSource")]
    pub date_source: Option<String>,
    /// `yyyymmdd` when `dateSource=manual`.
    #[serde(default)]
    pub date: Option<String>,
    /// Same-day retention slot; default `0` (overwrite same-day generation).
    #[serde(default)]
    pub fixver: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceBuildConfig {
    #[serde(default, rename = "retainBuildGenerations")]
    pub retain_build_generations: Option<u32>,
    /// Build generation ids that must not be auto-deleted by `build clean`.
    #[serde(default, rename = "protectBuildGenerations")]
    pub protect_build_generations: Vec<String>,
    #[serde(default)]
    pub generation: WorkspaceBuildGenerationConfig,
}

/// Prebuild / compile 范围过滤：按 target 路径 glob 与 scene export id 选择编译对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileScopeFilterConfig {
    /// 仅编译匹配的 target（相对 app `src/`，如 `scenes/home.mei`）。空 = 不限（仍受 exclude 约束）。
    #[serde(default, rename = "includeTargets")]
    pub include_targets: Vec<String>,
    /// 排除的 target glob，如 `**/*.page.mei`、`scenes/_shared/**`。
    #[serde(default, rename = "excludeTargets")]
    pub exclude_targets: Vec<String>,
    /// 排除的 scene export id glob，如 `*_analytics_page`。
    #[serde(default, rename = "excludeSceneIds")]
    pub exclude_scene_ids: Vec<String>,
    /// 为 true 时不把 discover 展开项加入 compile 队列（仅同步 MRG navigation）。
    #[serde(default, rename = "skipDiscover")]
    pub skip_discover: Option<bool>,
    /// 为 true 时不把 T2 page autogen 推导的 `.page.mei` focus 注入 manifest。
    #[serde(default, rename = "skipT2PageAutogenFocus")]
    pub skip_t2_page_autogen_focus: Option<bool>,
}

impl CompileScopeFilterConfig {
    pub fn is_active(&self) -> bool {
        !self.include_targets.is_empty()
            || !self.exclude_targets.is_empty()
            || !self.exclude_scene_ids.is_empty()
            || self.skip_discover == Some(true)
            || self.skip_t2_page_autogen_focus == Some(true)
    }

    pub fn should_skip_discover(&self) -> bool {
        self.skip_discover.unwrap_or(false)
    }

    pub fn should_skip_t2_page_autogen_focus(&self) -> bool {
        self.skip_t2_page_autogen_focus.unwrap_or(false)
    }
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
    /// 开发态选择性预热 / 求值（0535）；生产配置应省略或 `profile=full`。
    #[serde(
        default,
        rename = "devEval",
        skip_serializing_if = "WorkspaceDeployDevEvalConfig::is_empty"
    )]
    pub dev_eval: WorkspaceDeployDevEvalConfig,
    /// 统一运行计划；新配置优先于 `devEval`，旧字段仍原样反序列化/序列化。
    #[serde(
        default,
        rename = "runtimePlan",
        skip_serializing_if = "Option::is_none"
    )]
    pub runtime_plan: Option<RuntimePlan>,
}

/// 运行时目标模式。序列化值是稳定的客户端/服务端协议。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    #[default]
    Hot,
    Lazy,
    Frozen,
}

impl RuntimeMode {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Lazy => "lazy",
            Self::Frozen => "frozen",
        }
    }
}

/// Workspace 级运行计划。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePlan {
    #[serde(rename = "defaultMode")]
    pub default_mode: RuntimeMode,
    #[serde(default)]
    pub apps: BTreeMap<String, RuntimePlanApp>,
}

impl Default for RuntimePlan {
    fn default() -> Self {
        Self {
            default_mode: RuntimeMode::Hot,
            apps: BTreeMap::new(),
        }
    }
}

impl RuntimePlan {
    pub fn validate(&self) -> Result<()> {
        for (app_id, app) in &self.apps {
            let app_id = app_id.trim();
            if app_id.is_empty()
                || (app_id != "*"
                    && !app_id.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    }))
            {
                anyhow::bail!("deploy.runtimePlan.apps contains invalid app id `{app_id}`");
            }
            let mut scopes = std::collections::BTreeSet::new();
            for target in &app.targets {
                let scope = target.scope.trim().trim_matches('/');
                if scope.is_empty() {
                    anyhow::bail!(
                        "deploy.runtimePlan.apps.{app_id}.targets contains an empty scope"
                    );
                }
                if !scopes.insert(scope.to_string()) {
                    anyhow::bail!(
                        "deploy.runtimePlan.apps.{app_id}.targets contains duplicate scope `{scope}`"
                    );
                }
            }
            for metric_id in app.metric_overrides.keys() {
                if metric_id.trim().is_empty() || metric_id.chars().any(char::is_control) {
                    anyhow::bail!(
                        "deploy.runtimePlan.apps.{app_id}.metricOverrides contains invalid metric id"
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimePlanApp {
    pub targets: Vec<RuntimePlanTarget>,
    #[serde(rename = "metricOverrides")]
    pub metric_overrides: BTreeMap<String, RuntimeMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePlanTarget {
    pub scope: String,
    pub mode: RuntimeMode,
}

/// `deploy.devEval`：由 workspace config 驱动 `MEI_DEV_EVAL_*`。
///
/// 0535 双集合语义：
/// - `warmupScopes`：允许参与启动 / rewarm 预热的 scope 前缀（空 = 跳过启动 warmup）
/// - `evalScopes`：允许客户端动态求值（bind / eval-pack）的 scope 前缀（空 = 全部 placeholder）
/// - `scopes`：向后兼容；未设 `evalScopes` 时回退为 `evalScopes`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDeployDevEvalConfig {
    /// `full` | `static` | `scoped`
    #[serde(default)]
    pub profile: Option<String>,
    /// `preview_scope` 前缀列表（向后兼容；未设 `evalScopes` 时作为 eval 集合）
    #[serde(default)]
    pub scopes: Vec<String>,
    /// 允许预热的 scope 前缀（`scoped` 时；空 = 跳过启动 warmup）
    #[serde(default, rename = "warmupScopes")]
    pub warmup_scopes: Vec<String>,
    /// 允许动态求值的 scope 前缀（`scoped` 时；空 = 全部 placeholder）
    #[serde(default, rename = "evalScopes")]
    pub eval_scopes: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl WorkspaceDeployDevEvalConfig {
    pub fn is_empty(&self) -> bool {
        self.profile
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
            && self.scopes.is_empty()
            && self.warmup_scopes.is_empty()
            && self.eval_scopes.is_empty()
            && self
                .note
                .as_ref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
    }

    /// 将旧 `devEval` 双集合语义映射为统一三态计划。
    pub fn to_runtime_plan(&self) -> RuntimePlan {
        let profile = self
            .profile
            .as_deref()
            .unwrap_or("full")
            .trim()
            .to_ascii_lowercase();
        match profile.as_str() {
            "static" | "off" | "none" => RuntimePlan {
                default_mode: RuntimeMode::Frozen,
                apps: BTreeMap::new(),
            },
            "scoped" | "scope" | "focused" => {
                let eval_scopes = if self.eval_scopes.is_empty() {
                    &self.scopes
                } else {
                    &self.eval_scopes
                };
                let mut targets = BTreeMap::<String, RuntimeMode>::new();
                for scope in eval_scopes {
                    let scope = scope.trim().trim_matches('/');
                    if !scope.is_empty() {
                        targets.insert(scope.to_string(), RuntimeMode::Lazy);
                    }
                }
                for scope in &self.warmup_scopes {
                    let scope = scope.trim().trim_matches('/');
                    if !scope.is_empty() {
                        targets.insert(scope.to_string(), RuntimeMode::Hot);
                    }
                }
                let app = RuntimePlanApp {
                    targets: targets
                        .into_iter()
                        .map(|(scope, mode)| RuntimePlanTarget { scope, mode })
                        .collect(),
                    metric_overrides: BTreeMap::new(),
                };
                RuntimePlan {
                    default_mode: RuntimeMode::Frozen,
                    apps: BTreeMap::from([("*".to_string(), app)]),
                }
            }
            _ => RuntimePlan::default(),
        }
    }
}

impl WorkspaceDeployConfig {
    /// 新 `runtimePlan` 优先；未配置时兼容映射旧 `devEval`。
    pub fn effective_runtime_plan(&self) -> RuntimePlan {
        if let Some(runtime_plan) = &self.runtime_plan {
            runtime_plan.clone()
        } else if !self.dev_eval.is_empty() {
            self.dev_eval.to_runtime_plan()
        } else {
            RuntimePlan::default()
        }
    }
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
    /// Prebuild 编译范围：include/exclude target 与 scene export，用于首页-only 等快速验证。
    #[serde(default, rename = "compileScope")]
    pub compile_scope: Option<CompileScopeFilterConfig>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default, rename = "compileScope")]
    pub compile_scope: Option<CompileScopeFilterConfig>,
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
            && self.jwt_secret.as_deref().unwrap_or("").trim().is_empty()
            && self.cookie_name.as_deref().unwrap_or("").trim().is_empty()
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
