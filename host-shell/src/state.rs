use std::sync::{Arc, RwLock};

use axum::extract::FromRef;
use mei_host_auth::AuthServeState;
use mei_host_core::HostContext;

use crate::build_ops::OpsJobState;

#[derive(Debug, Clone)]
pub struct ShellState {
    pub ctx: HostContext,
    pub package_root: std::path::PathBuf,
    pub plug_ds_endpoint: String,
    pub plug_ds_managed: bool,
    pub imported: bool,
    pub warmed_up: bool,
    pub host_started_at_ms: u64,
    pub ops_job: Option<OpsJobState>,
    pub last_ops_job: Option<OpsJobState>,
}

impl ShellState {
    pub fn new(
        workspace: std::path::PathBuf,
        app_id: String,
        package_root: std::path::PathBuf,
        plug_ds_endpoint: String,
        plug_ds_managed: bool,
    ) -> Self {
        Self {
            ctx: HostContext::new(workspace, app_id),
            package_root,
            plug_ds_endpoint,
            plug_ds_managed,
            imported: false,
            warmed_up: false,
            host_started_at_ms: current_time_ms(),
            ops_job: None,
            last_ops_job: None,
        }
    }
}

pub type SharedState = Arc<RwLock<ShellState>>;

#[derive(Clone)]
pub struct HostHttpState {
    pub shell: SharedState,
    pub auth: AuthServeState,
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
