//! Shared imports for workspace_stock submodules.

pub(crate) use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub(crate) use anyhow::{Context, Result};
pub(crate) use chrono::Utc;
pub(crate) use mei_lang_kernel::{
    resolve_authoring_root, resolve_components_root, resolve_stock_root, resolve_templates_root,
    resolve_toolchain_root, resolve_workspace_runtime_root, stock_authoring_source,
    stock_components_source, stock_templates_source, workspace_config_path, write_workspace_config,
    WorkspaceConfig, WorkspacePathsConfig, WorkspaceProfile, WorkspaceStockBootstrapConfig,
    WorkspaceStockCatalogAppConfig, WorkspaceStockCatalogConfig, WorkspaceStockCatalogKindConfig,
    WorkspaceStockConfig, WorkspaceStockPreviewConfig, APP_CONFIG_FILENAME, DEFAULT_APPS_REL,
    DEFAULT_STOCK_AUTHORING_REL, DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL,
    WORKSPACE_HOSTS_DIR_REL,
};
pub(crate) use walkdir::WalkDir;

pub(crate) use super::types::*;
