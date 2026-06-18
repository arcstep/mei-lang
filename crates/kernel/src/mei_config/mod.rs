//! App 级 `.mei-config.json` 与 workspace 级 `.mei-workspace.json` 分层加载。
//!
//! `.mei` 真源只读；宿主仅通过 ops 白名单对象写回配置，不写 `.mei`。

mod auth_bundle;
mod authoring_helpers;
mod authoring_policy;
mod io;
mod ops;
mod types;
mod workspace_paths;

#[cfg(test)]
mod tests;

pub use authoring_helpers::{
    resolve_authoring_helpers, AuthoringHelpers,
};
pub use authoring_policy::{forbidden_authoring_tokens, validate_authoring_policy};
pub use auth_bundle::{
    load_workspace_auth_bundle, workspace_auth_config_path, workspace_auth_host_id,
    workspace_auth_state_dir, write_workspace_auth_bundle, WorkspaceAuthBundle,
};
pub use io::{
    load_mei_config_for_app, load_workspace_config, resolve_app_entry_main, resolve_app_main_path,
    resolve_mei_config_path, write_mei_config, write_workspace_config,
};
pub use ops::{merge_ops_section, OpsConfigPatch};
pub use types::{
    AppEntryConfig, AppFeaturesConfig, AppPathsConfig, AuthKeyPairConfig, AuthUserConfig,
    DiscoverConfig, FileCacheConfig, FileCacheSettings, MeiConfig, OpsBasemapEntry, OpsConfig,
    OpsSourceEntry, RuntimeConfig, RuntimeWarmupApp, RuntimeWarmupDatasetRequest,
    RuntimeWarmupManifest, RuntimeWarmupXlsxSource, WorkspaceAuthConfig, WorkspaceComplianceConfig, WorkspaceConfig,
    WorkspaceHostState, WorkspacePathsConfig, WorkspaceProfile, WorkspaceWarmupAppConfig,
    WorkspaceWarmupConfig, WorkspaceWarmupDatasetConfig, WorkspaceWarmupXlsxConfig,
    AUTH_JOURNAL_REL_PATH,
    LEGACY_WORKSPACE_AGENT_DB_REL, LEGACY_WORKSPACE_SNAPSHOT_DIR_REL,
    LEGACY_WORKSPACE_SNAPSHOT_GIT_REL, PRE_LOCAL_AUTH_JOURNAL_REL_PATH,
    WORKSPACE_AGENT_DB_REL, WORKSPACE_AGENT_LOCAL_DIR_REL, WORKSPACE_AUTH_DIR_REL,
    DEFAULT_APP_ENTRY_MAIN, DEFAULT_HOST_STATE_ID, DEFAULT_STOCK_AUTHORING_REL,
    DEFAULT_STOCK_COMPONENTS_REL,
    DEFAULT_STOCK_TEMPLATES_REL, LEGACY_AUTH_JOURNAL_REL_PATH, MEI_CONFIG_FILENAME,
    MEI_WORKSPACE_CONFIG_FILENAME, OPS_JOURNAL_REL_PATH, OPS_OBJECT_KINDS,
    WORKSPACE_HOSTS_DIR_REL, WORKSPACE_HOST_STATE_SCHEMA_VERSION, WORKSPACE_LOCAL_DIR_REL,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL, WORKSPACE_SNAPSHOT_DIR_REL,
    WORKSPACE_SNAPSHOT_GIT_REL,
};
pub use workspace_paths::{
    app_mei_config_path, is_app_config_root, resolve_app_root, resolve_authoring_root,
    resolve_components_root,
    resolve_templates_root, set_mei_package_root, stock_components_source, stock_templates_source,
    workspace_config_path,
};
