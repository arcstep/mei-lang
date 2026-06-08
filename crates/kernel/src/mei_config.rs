//! App 级 `.mei-config.json` 与 workspace 级 `.mei-workspace.json` 分层加载。
//!
//! `.mei` 真源只读；宿主仅通过 ops 白名单对象写回配置，不写 `.mei`。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MEI_CONFIG_FILENAME: &str = ".mei-config.json";
pub const MEI_WORKSPACE_CONFIG_FILENAME: &str = ".mei-workspace.json";
pub const OPS_JOURNAL_REL_PATH: &str = "ops/.mei-ops-journal.json";
pub const AUTH_JOURNAL_REL_PATH: &str = ".mei/auth/auth-journal.json";
pub const LEGACY_AUTH_JOURNAL_REL_PATH: &str = "auth/.mei-auth-journal.json";
/// 可 Git 跟踪的物化组件库（与运行时 `.mei/` 分离）。
pub const DEFAULT_STOCK_COMPONENTS_REL: &str = ".stock/components";
/// 可 Git 跟踪的物化模板库。
pub const DEFAULT_STOCK_TEMPLATES_REL: &str = ".stock/templates";
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
    /// 登录页与底栏展示的备案号、版权等合规信息。
    #[serde(default)]
    pub compliance: WorkspaceComplianceConfig,
    /// 工作区级宿主认证配置（用户清单、JWT、登录加密密钥）。
    #[serde(default)]
    pub auth: WorkspaceAuthConfig,
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
    #[serde(default)]
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspacePathsConfig {
    /// 组件库根（相对 profile 根或绝对路径）；未设且未物化时回退 `mei-lang/stock/components`。
    #[serde(default)]
    pub components: Option<String>,
    /// 模板库根；未设且未物化时回退 `mei-lang/stock/templates`。
    #[serde(default)]
    pub templates: Option<String>,
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

pub fn is_app_config_root(dir: &Path) -> bool {
    dir.join(MEI_CONFIG_FILENAME).is_file()
}

pub fn app_mei_config_path(app_root: &Path) -> PathBuf {
    app_root.join(MEI_CONFIG_FILENAME)
}

pub fn workspace_config_path(segment_root: &Path) -> PathBuf {
    segment_root.join(MEI_WORKSPACE_CONFIG_FILENAME)
}

static MEI_PACKAGE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// 由 `mei-lang-server` 启动时注入；供 stock 回退路径解析。
pub fn set_mei_package_root(path: PathBuf) {
    let _ = MEI_PACKAGE_ROOT.set(path);
}

fn mei_package_root() -> Option<&'static Path> {
    MEI_PACKAGE_ROOT.get().map(|path| path.as_path())
}

fn resolve_workspace_path(source_root: &Path, rel: &str) -> PathBuf {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return source_root.to_path_buf();
    }
    if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        source_root.join(trimmed)
    }
}

fn configured_components_rel(cfg: &WorkspaceConfig) -> Option<&str> {
    cfg.paths
        .components
        .as_deref()
        .or(cfg.discover.components_root.as_deref())
        .filter(|value| !value.trim().is_empty())
}

fn configured_templates_rel(cfg: &WorkspaceConfig) -> Option<&str> {
    cfg.paths
        .templates
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

/// 解析组件根：`paths.components` → 物化目录 → `mei-lang/stock/components`。
pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_components_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let materialized = resolve_workspace_path(source_root, DEFAULT_STOCK_COMPONENTS_REL);
    if materialized.is_dir() {
        return materialized;
    }
    if let Some(package_root) = mei_package_root() {
        let stock = package_root.join("stock/components");
        if stock.is_dir() {
            return stock;
        }
    }
    materialized
}

/// 解析模板根：`paths.templates` → 物化目录 → `mei-lang/stock/templates`。
pub fn resolve_templates_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_templates_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let materialized = resolve_workspace_path(source_root, DEFAULT_STOCK_TEMPLATES_REL);
    if materialized.is_dir() {
        return materialized;
    }
    if let Some(package_root) = mei_package_root() {
        let stock = package_root.join("stock/templates");
        if stock.is_dir() {
            return stock;
        }
    }
    materialized
}

pub fn stock_components_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/components")
}

pub fn stock_templates_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/templates")
}

/// 将 CLI/URL 中的 `app_id` 解析为应用目录（支持 `discover.appAliases`）。
pub fn resolve_app_root(source_root: &Path, app_id: &str) -> PathBuf {
    let app_id = app_id.trim();
    let direct = source_root.join(app_id);
    if direct.exists() {
        return direct;
    }
    let cfg = load_workspace_config(source_root);
    if let Some(alias) = cfg.discover.app_aliases.get(app_id) {
        let target = alias.trim();
        if !target.is_empty() {
            let aliased = source_root.join(target);
            if aliased.exists() {
                return aliased;
            }
        }
    }
    direct
}

/// 工作区 segment 根目录的 `.mei-workspace.json`。
pub fn workspace_auth_config_path(segment_root: &Path) -> PathBuf {
    workspace_config_path(segment_root)
}

#[derive(Debug, Clone)]
pub struct WorkspaceAuthBundle {
    pub auth: WorkspaceAuthConfig,
    pub config_path: PathBuf,
}

fn workspace_auth_section_empty(auth: &WorkspaceAuthConfig) -> bool {
    auth.users.is_empty()
        && auth
            .jwt_secret
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && auth.key_pair.public_key_pem.trim().is_empty()
        && auth.key_pair.private_key_pem.trim().is_empty()
}

/// 读取工作区认证配置：优先 `{segment_root}/.mei-workspace.json#auth`，
/// 如为空则兼容回退到同级误写入的 `.mei-config.json#auth`。
pub fn load_workspace_auth_bundle(segment_root: &Path) -> WorkspaceAuthBundle {
    let config_path = workspace_auth_config_path(segment_root);
    let mut auth = if config_path.is_file() {
        WorkspaceConfig::load_or_default(&config_path).auth
    } else {
        WorkspaceAuthConfig::default()
    };
    if workspace_auth_section_empty(&auth) {
        let misplaced_path = segment_root.join(MEI_CONFIG_FILENAME);
        if misplaced_path.is_file() {
            let misplaced_auth = MeiConfig::load_or_default(&misplaced_path).auth;
            if !workspace_auth_section_empty(&misplaced_auth) {
                auth = misplaced_auth;
            }
        }
    }
    WorkspaceAuthBundle {
        auth,
        config_path,
    }
}

/// 将认证段写入工作区根 `.mei-workspace.json`。
pub fn write_workspace_auth_bundle(segment_root: &Path, auth: &WorkspaceAuthConfig) -> Result<PathBuf> {
    let path = workspace_auth_config_path(segment_root);
    let mut config = if path.is_file() {
        WorkspaceConfig::load_or_default(&path)
    } else {
        WorkspaceConfig::default()
    };
    if config.schema_version == 0 {
        config.schema_version = 1;
    }
    config.auth = auth.clone();
    write_workspace_config(&path, &config)?;
    Ok(path)
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
            workspace: WorkspaceProfile::default(),
            paths: WorkspacePathsConfig::default(),
            discover: legacy_app.discover,
            menu: legacy_app.menu,
            runtime: legacy_app.runtime,
            compliance: WorkspaceComplianceConfig::default(),
            auth: WorkspaceAuthConfig::default(),
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
    write_string_atomically(path, raw.as_str())
        .with_context(|| format!("failed to write mei config {}", path.display()))
}

pub fn write_workspace_config(path: &Path, config: &WorkspaceConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create workspace config parent dir {}",
                parent.display()
            )
        })?;
    }
    let raw =
        serde_json::to_string_pretty(config).context("failed to serialize workspace config")?;
    write_string_atomically(path, raw.as_str())
        .with_context(|| format!("failed to write workspace config {}", path.display()))
}

fn write_string_atomically(path: &Path, raw: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path {} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(
        ".{file_name}.tmp-{}-{stamp}",
        std::process::id()
    ));
    fs::write(&tmp_path, raw)
        .with_context(|| format!("failed to write temporary file {}", tmp_path.display()))?;
    if let Err(error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to atomically replace {} from {}",
                path.display(),
                tmp_path.display()
            )
        });
    }
    Ok(())
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
    fn workspace_compliance_deserializes_from_json() {
        let raw = r#"{
            "compliance": {
                "icpRecord": "渝ICP备12345678号",
                "psbRecord": "渝公网安备 12345678号",
                "copyright": "示例主体"
            }
        }"#;
        let cfg: WorkspaceConfig = serde_json::from_str(raw).expect("parse compliance");
        assert_eq!(
            cfg.compliance.icp_record_trimmed(),
            Some("渝ICP备12345678号")
        );
        assert_eq!(
            cfg.compliance.psb_record_trimmed(),
            Some("渝公网安备 12345678号")
        );
        assert_eq!(cfg.compliance.copyright_trimmed(), Some("示例主体"));
    }

    #[test]
    fn workspace_discover_skip_normalizes_segments() {
        let cfg = WorkspaceConfig {
            discover: DiscoverConfig {
                skip_directories: vec![" /foo/ ".into(), "nested/bad".into(), "ok".into()],
                ..Default::default()
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

    #[test]
    fn workspace_auth_bundle_reads_workspace_json() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-workspace-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let workspace = WorkspaceConfig {
            auth: WorkspaceAuthConfig {
                jwt_secret: Some("workspace-secret".to_string()),
                users: vec![AuthUserConfig {
                    username: "guest01".to_string(),
                    password_hash: "$argon2id$v=19$workspace".to_string(),
                    roles: vec!["guest".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        write_workspace_config(&workspace_config_path(&dir), &workspace).expect("write workspace");
        let bundle = load_workspace_auth_bundle(&dir);
        assert_eq!(bundle.auth.jwt_secret.as_deref(), Some("workspace-secret"));
        assert_eq!(bundle.auth.users.len(), 1);
        assert_eq!(bundle.auth.users[0].username, "guest01");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_auth_bundle_reads_misplaced_mei_config_for_migration() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-misplaced-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let mut config = MeiConfig::default();
        config.auth.jwt_secret = Some("misplaced-secret".to_string());
        config.auth.users.push(AuthUserConfig {
            username: "admin".to_string(),
            password_hash: "$argon2id$v=19$misplaced".to_string(),
            roles: vec!["admin".to_string()],
            ..Default::default()
        });
        write_mei_config(&dir.join(MEI_CONFIG_FILENAME), &config).expect("seed misplaced mei config");
        let bundle = load_workspace_auth_bundle(&dir);
        assert_eq!(bundle.auth.jwt_secret.as_deref(), Some("misplaced-secret"));
        assert_eq!(bundle.auth.users.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_auth_bundle_writes_workspace_json_without_dropping_runtime() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-write-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let workspace = WorkspaceConfig {
            discover: DiscoverConfig {
                skip_directories: vec!["cache".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        write_workspace_config(&workspace_auth_config_path(&dir), &workspace)
            .expect("seed workspace config");
        let mut auth = WorkspaceAuthConfig::default();
        auth.jwt_secret = Some("jwt".to_string());
        auth.users.push(AuthUserConfig {
            username: "admin".to_string(),
            password_hash: "$argon2id$v=19$demo".to_string(),
            roles: vec!["admin".to_string()],
            ..Default::default()
        });
        write_workspace_auth_bundle(&dir, &auth).expect("write auth");
        let loaded = WorkspaceConfig::load_or_default(&workspace_auth_config_path(&dir));
        assert_eq!(loaded.discover.skip_directories, vec!["cache"]);
        assert_eq!(loaded.auth.users.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
