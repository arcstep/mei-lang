//! v2 build store: `build/store/{buildId}/`, workspace `deploy/state/links.json`, promote/rollback.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::mei_config::types::DEPLOY_LINKS_REL;
use crate::mei_config::workspace_paths::resolve_deploy_root;

pub const LINKS_STATE_SCHEMA: &str = "mei-workspace-links-v1";
pub const BUILD_MANIFEST_SCHEMA: &str = "mei-build-manifest-v1";
pub const DEV_TOOLCHAIN_VERSION: &str = "0.0.0-dev-local";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolchainLinks {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildLinks {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub candidate: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinksState {
    #[serde(rename = "schemaVersion", default = "default_links_schema")]
    pub schema_version: String,
    #[serde(default, rename = "sourceRevision")]
    pub source_revision: Option<String>,
    #[serde(default, rename = "stockRevision")]
    pub stock_revision: Option<String>,
    #[serde(default)]
    pub toolchain: ToolchainLinks,
    #[serde(default)]
    pub build: BuildLinks,
}

fn default_links_schema() -> String {
    LINKS_STATE_SCHEMA.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "buildId")]
    pub build_id: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "toolchainVersion")]
    pub toolchain_version: String,
    #[serde(default, rename = "sourceRevision")]
    pub source_revision: Option<String>,
    #[serde(default, rename = "stockRevision")]
    pub stock_revision: Option<String>,
    #[serde(rename = "finishedAt")]
    pub finished_at: String,
}

pub fn deploy_links_path(source_root: &Path) -> PathBuf {
    resolve_deploy_root(source_root).join(DEPLOY_LINKS_REL)
}

pub fn read_links_state(source_root: &Path) -> Result<LinksState> {
    let path = deploy_links_path(source_root);
    if !path.is_file() {
        return Ok(LinksState::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read deploy links {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse deploy links {}", path.display()))
}

pub fn write_links_state(source_root: &Path, links: &LinksState) -> Result<()> {
    let path = deploy_links_path(source_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = links.clone();
    if value.schema_version.is_empty() {
        value.schema_version = LINKS_STATE_SCHEMA.to_string();
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&value)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}
