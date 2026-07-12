use std::path::{Path, PathBuf};

use mei_lang_kernel::{resolve_app_registry_root, resolve_app_root};

use crate::config::{load_app_config, AppConfig};

/// Runtime host context shared by shell and plugins.
///
/// For pinned generation + instance identity (do not follow mutable `env/current`),
/// prefer [`crate::RuntimeContext`] via [`HostContext::with_runtime`].
#[derive(Debug, Clone)]
pub struct HostContext {
    pub workspace_root: PathBuf,
    pub app_id: String,
}

impl HostContext {
    pub fn new(workspace_root: impl Into<PathBuf>, app_id: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            app_id: app_id.into(),
        }
    }

    pub fn runtime_root(&self) -> PathBuf {
        self.workspace_root.join("runtime")
    }

    pub fn registry_root(&self) -> PathBuf {
        resolve_app_registry_root(self.app_root().as_path())
    }

    /// Deprecated alias: registry lives under `build/active/registry/`.
    pub fn platform_graph_root(&self) -> PathBuf {
        self.registry_root()
    }

    pub fn app_root(&self) -> PathBuf {
        resolve_app_root(self.workspace_root.as_path(), self.app_id.as_str())
    }

    pub fn bundle_path(&self) -> PathBuf {
        mei_bundle::default_bundle_path(self.workspace_root.as_path(), self.app_id.as_str())
    }
}

pub fn resolve_bundle_path(ctx: &HostContext, bundle: Option<&Path>) -> PathBuf {
    bundle
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ctx.bundle_path())
}

pub fn load_app_config_for_ctx(ctx: &HostContext) -> anyhow::Result<AppConfig> {
    load_app_config(ctx.app_root().as_path())
}
