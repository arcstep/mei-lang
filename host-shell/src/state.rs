use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use axum::extract::FromRef;
use mei_host_auth::AuthServeState;
use mei_host_core::HostContext;

use crate::build_ops::OpsJobState;
use crate::managed_plug::ManagedPlugDsPool;

/// Per-app lazy import state (avoids infinite retry when bundle is missing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMaterializationState {
    InProgress,
    Failed {
        message: String,
        missing_artifacts: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ShellState {
    pub ctx: HostContext,
    pub package_root: std::path::PathBuf,
    /// Default-app plug-ds endpoint (ops banner / backward compat).
    pub plug_ds_endpoint: String,
    pub plug_ds_by_app: BTreeMap<String, String>,
    pub plug_ds_managed: bool,
    pub imported: bool,
    pub warmed_up: bool,
    pub host_started_at_ms: u64,
    pub ops_job: Option<OpsJobState>,
    pub last_ops_job: Option<OpsJobState>,
    /// `preparing` / `waiting_artifacts` / `importing` / `plug_ds` / `priming_cache` / `ready` / `failed`
    pub startup_phase: String,
    pub startup_detail: Option<String>,
    pub startup_error: Option<String>,
    pub app_materialization: BTreeMap<String, AppMaterializationState>,
}

impl ShellState {
    pub fn new(
        workspace: std::path::PathBuf,
        app_id: String,
        package_root: std::path::PathBuf,
        plug_ds_by_app: BTreeMap<String, String>,
        plug_ds_managed: bool,
    ) -> Self {
        let plug_ds_endpoint = plug_ds_by_app
            .get(app_id.as_str())
            .cloned()
            .unwrap_or_default();
        Self {
            ctx: HostContext::new(workspace, app_id),
            package_root,
            plug_ds_endpoint,
            plug_ds_by_app,
            plug_ds_managed,
            imported: false,
            warmed_up: false,
            host_started_at_ms: current_time_ms(),
            ops_job: None,
            last_ops_job: None,
            startup_phase: "preparing".to_string(),
            startup_detail: Some("正在启动 MeiLang 宿主服务…".to_string()),
            startup_error: None,
            app_materialization: BTreeMap::new(),
        }
    }

    pub fn host_ctx_for_app(&self, app_id: &str) -> HostContext {
        HostContext::new(self.ctx.workspace_root.clone(), app_id.to_string())
    }

    pub fn plug_ds_endpoint_for(&self, app_id: &str) -> Option<&str> {
        self.plug_ds_by_app.get(app_id).map(String::as_str)
    }
}

pub type SharedState = Arc<RwLock<ShellState>>;

#[derive(Clone)]
pub struct HostHttpState {
    pub shell: SharedState,
    pub auth: AuthServeState,
    pub managed_plug: Arc<Mutex<Option<ManagedPlugDsPool>>>,
}

impl FromRef<HostHttpState> for AuthServeState {
    fn from_ref(input: &HostHttpState) -> Self {
        input.auth.clone()
    }
}

impl FromRef<HostHttpState> for SharedState {
    fn from_ref(input: &HostHttpState) -> Self {
        input.shell.clone()
    }
}

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn plug_ds_endpoint_for_routes_per_app() {
        let mut plug_ds_by_app = BTreeMap::new();
        plug_ds_by_app.insert("data-demo".to_string(), "http://127.0.0.1:9001".to_string());
        plug_ds_by_app.insert("mini-park".to_string(), "http://127.0.0.1:9002".to_string());
        let state = ShellState::new(
            Path::new("/tmp/ws").to_path_buf(),
            "data-demo".to_string(),
            Path::new("/tmp/pkg").to_path_buf(),
            plug_ds_by_app,
            false,
        );
        assert_eq!(
            state.plug_ds_endpoint_for("data-demo"),
            Some("http://127.0.0.1:9001")
        );
        assert_eq!(
            state.plug_ds_endpoint_for("mini-park"),
            Some("http://127.0.0.1:9002")
        );
        assert_eq!(state.plug_ds_endpoint_for("missing"), None);
    }

    #[test]
    fn host_ctx_for_app_uses_requested_app_id() {
        let state = ShellState::new(
            Path::new("/tmp/ws").to_path_buf(),
            "data-demo".to_string(),
            Path::new("/tmp/pkg").to_path_buf(),
            BTreeMap::new(),
            false,
        );
        let ctx = state.host_ctx_for_app("mini-park");
        assert_eq!(ctx.app_id, "mini-park");
        assert_eq!(ctx.workspace_root, Path::new("/tmp/ws"));
    }
}
