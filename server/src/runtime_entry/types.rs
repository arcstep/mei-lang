use super::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFlavor {
    Compat,
    Toolchain,
    HostWeb,
}

impl BinaryFlavor {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            BinaryFlavor::Compat => "mei",
            BinaryFlavor::Toolchain => "mei-toolchain",
            BinaryFlavor::HostWeb => "mei-host-web",
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) package_root: Arc<PathBuf>,
    pub(crate) source_root: Arc<PathBuf>,
    pub(crate) agent_preferred_mode: Arc<String>,
    pub(crate) agent_preferred_server_url: Arc<String>,
    pub(crate) agent_auto_start: bool,
    pub(crate) auth_enforcement: mei_host_auth::AuthEnforcement,
    pub(crate) agent_runtime: Arc<Mutex<crate::agent_runtime::ManagedOpencodeRuntime>>,
    pub(crate) agent_session_context: Arc<Mutex<HashMap<String, SessionContextSnapshot>>>,
    pub(crate) native_agent: Arc<crate::mei_agent::NativeAgent>,
}

impl axum::extract::FromRef<AppState> for mei_host_auth::AuthServeState {
    fn from_ref(app: &AppState) -> mei_host_auth::AuthServeState {
        mei_host_auth::AuthServeState {
            source_root: app.source_root.clone(),
            auth_enforcement: app.auth_enforcement,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SessionContextSnapshot {
    pub signature: String,
    pub context: String,
}
