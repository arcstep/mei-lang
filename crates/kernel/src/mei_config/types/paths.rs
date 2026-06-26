

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

