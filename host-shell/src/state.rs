use std::sync::{Arc, RwLock};

use mei_host_core::HostContext;

#[derive(Debug, Clone)]
pub struct ShellState {
    pub ctx: HostContext,
    pub package_root: std::path::PathBuf,
    pub imported: bool,
    pub warmed_up: bool,
    pub host_started_at_ms: u64,
}

impl ShellState {
    pub fn new(workspace: std::path::PathBuf, app_id: String, package_root: std::path::PathBuf) -> Self {
        Self {
            ctx: HostContext::new(workspace, app_id),
            package_root,
            imported: false,
            warmed_up: false,
            host_started_at_ms: current_time_ms(),
        }
    }
}

pub type SharedState = Arc<RwLock<ShellState>>;

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
