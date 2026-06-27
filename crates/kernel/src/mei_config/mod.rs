//! App 级 `.mei-config.json` 与 workspace 级 `.mei-workspace.json` 分层加载。
//!
//! `.mei` 真源只读；宿主仅通过 ops 白名单对象写回配置，不写 `.mei`。

mod auth_bundle;
mod authoring_helpers;
mod authoring_policy;
mod build_store;
mod io;
mod ops;
mod shell_theme;
mod stock_catalog;
mod theme_overlay;
mod types;
mod workspace_paths;

#[cfg(test)]
mod tests;

pub use build_store::{
    app_build_store_dir, app_var_store_dir, apply_toolchain_store_symlinks, begin_prebuild_generation,
    clear_prebuild_build_root_override, finish_prebuild_generation, generate_build_id,
    migrate_legacy_app_mei, migrate_legacy_workspace_mei, promote_build, read_build_manifest,
    read_links_state, record_toolchain_install_links, resolve_active_build_id,
    resolve_toolchain_version, rollback_build, restore_prebuild_build_root_override,
    set_prebuild_build_root_override, snapshot_prebuild_build_root_override,
    toolchain_store_dir, write_build_manifest, write_links_state, BuildLinks, BuildManifest,
    LinksState, PrebuildGeneration, ToolchainLinks, BUILD_MANIFEST_SCHEMA, DEV_TOOLCHAIN_VERSION,
    LINKS_STATE_SCHEMA,
};
pub use auth_bundle::{
    load_workspace_auth_bundle, workspace_auth_config_path, workspace_auth_host_id,
    workspace_auth_state_dir, write_workspace_auth_bundle, WorkspaceAuthBundle,
};
pub use authoring_helpers::{resolve_authoring_helpers, AuthoringHelpers};
pub use authoring_policy::{forbidden_authoring_tokens, validate_authoring_policy};
pub use io::{
    load_mei_config_for_app, load_workspace_config, resolve_app_entry_main, resolve_app_main_path,
    resolve_mei_config_path, write_mei_config, write_workspace_config,
};
pub use ops::{merge_ops_section, OpsConfigPatch};
pub use shell_theme::{resolve_workspace_shell_theme, validate_workspace_shell_theme};
pub use theme_overlay::{
    mei_config_compile_revision_digest, ops_themes_revision_digest, resolve_live_ops_theme_value,
};
pub use types::{
    AccessAiExternalConfig, AppEntryConfig, AppFeaturesConfig, AppPathsConfig, AuthKeyPairConfig,
    AuthUserConfig,
    DiscoverConfig, FileCacheConfig, FileCacheSettings, MeiConfig, OpsBasemapEntry, OpsConfig,
    OpsSourceEntry, RuntimeConfig, RuntimeWarmupApp, RuntimeWarmupDatasetRequest,
    RuntimeWarmupManifest, RuntimeWarmupXlsxSource, WorkspaceAuthConfig, WorkspaceComplianceConfig,
    WorkspaceConfig, WorkspaceHostState, WorkspaceOpsConfig, WorkspacePathsConfig,
    WorkspaceProfile,     WorkspaceStockBootstrapConfig, WorkspaceStockCatalogAppConfig, WorkspaceStockCatalogConfig,
    WorkspaceStockCatalogKindConfig, WorkspaceStockConfig, WorkspaceStockPreviewConfig,
    WorkspaceStockSourceEntry, DEFAULT_STOCK_CATALOG_APP_ID, WorkspaceWarmupAppConfig,
    WorkspaceWarmupConfig,
    WorkspaceWarmupDatasetConfig, WorkspaceWarmupXlsxConfig, WorkspaceBuildConfig,
    CompileScopeFilterConfig, WorkspaceToolchainConfig, APP_CONFIG_FILENAME, APP_BUILD_STORE_REL, APP_VAR_STORE_REL,
    BUILD_MANIFEST_FILENAME, DEPLOY_LINKS_REL, PREBUILD_COMPILE_INDEX_REL, PREBUILD_DIR_REL,
    PREBUILD_LAST_BUILD_SUMMARY_REL, TOOLCHAIN_ACTIVE_REL, TOOLCHAIN_STORE_REL,
    AUTH_JOURNAL_REL_PATH, DEFAULT_APPS_REL, DEFAULT_APP_ENTRY_MAIN, DEFAULT_HOST_STATE_ID, DEFAULT_STOCK_AUTHORING_REL,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, LEGACY_AUTH_JOURNAL_REL_PATH,
    LEGACY_WORKSPACE_AGENT_DB_REL, LEGACY_WORKSPACE_SNAPSHOT_DIR_REL,
    LEGACY_WORKSPACE_SNAPSHOT_GIT_REL, MEI_CONFIG_FILENAME, MEI_WORKSPACE_CONFIG_FILENAME,
    OPS_JOURNAL_REL_PATH, OPS_OBJECT_KINDS, PRE_LOCAL_AUTH_JOURNAL_REL_PATH,
    WORKSPACE_AGENT_DB_REL, WORKSPACE_AGENT_LOCAL_DIR_REL, WORKSPACE_AUTH_DIR_REL,
    WORKSPACE_CONFIG_FILENAME, WORKSPACE_HOSTS_DIR_REL, WORKSPACE_HOST_STATE_SCHEMA_VERSION, WORKSPACE_LOCAL_DIR_REL,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL, WORKSPACE_SNAPSHOT_DIR_REL, WORKSPACE_SNAPSHOT_GIT_REL,
};
pub use stock_catalog::{
    is_stock_catalog_app, is_stock_catalog_app_for_root, match_path_glob,
    normalize_stock_relative_path, stock_catalog_app_config, stock_catalog_app_id,
    stock_catalog_enabled, stock_path_excluded, StockCatalogKind,
};
pub use workspace_paths::{
    app_mei_config_path, app_source_rel_path_lookup_keys, canonical_app_source_rel_path,
    is_v2_app_root, resolve_app_build_root, resolve_app_build_store_root,
    resolve_app_id, resolve_app_mei_file_path, resolve_app_mei_store_root, resolve_app_root,
    resolve_app_data_snapshot_root, resolve_app_eval_cache_root, resolve_app_registry_root,
    resolve_app_src_root, resolve_app_var_root, resolve_apps_root, resolve_authoring_root,
    is_app_mei_source_rel, resolve_components_root, resolve_deploy_root, resolve_templates_root,
    resolve_toolchain_root, resolve_workspace_cache_root, resolve_workspace_graph_root,
    resolve_workspace_logs_root, resolve_workspace_platform_root, resolve_workspace_path,
    resolve_workspace_runtime_root, resolve_workspace_source_root_from_app_root, resolve_stock_root,
    set_mei_package_root, stock_authoring_source, stock_components_source, stock_templates_source,
    workspace_config_path,
};
