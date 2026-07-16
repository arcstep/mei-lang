use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex, RwLock,
};

use axum::extract::FromRef;
use mei_host_auth::{AuthPrincipal, AuthServeState};
use mei_host_core::{HostContext, LaunchManifest, RouteBinding};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

use mei_lang_kernel::DataModeCeiling;

use crate::app_runtime_proxy::RuntimeProxyIdentity;
use crate::app_runtime_supervisor::SharedAppRuntime;
use crate::app_start_inflight::AppStartInflight;
use crate::build_ops::OpsJobState;
use crate::managed_plug::ManagedPlugDsPool;
use crate::runtime_actor::RuntimeActorHandle;
use crate::runtime_route_table::{RuntimeRouteEntry, RuntimeRoutePhase, SharedRuntimeRouteTable};

pub const HOST_EVENT_CAPACITY: usize = 128;

#[derive(Debug, Clone)]
pub struct CleanupPreviewState {
    pub token: String,
    pub revision: String,
    pub generated_at_ms: u64,
    pub report: mei_lang_kernel::CleanEnvReport,
    /// When set, execute only cleans these apps (card-scoped cleanup).
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub emitted_at_ms: u64,
    pub payload: Value,
}

impl HostEvent {
    pub fn new(event_type: impl Into<String>, payload: Value) -> Self {
        Self {
            event_type: event_type.into(),
            emitted_at_ms: current_time_ms(),
            payload,
        }
    }
}

pub fn host_event_channel() -> broadcast::Sender<HostEvent> {
    let (sender, _) = broadcast::channel(HOST_EVENT_CAPACITY);
    sender
}

#[derive(Debug, Default)]
pub struct HostEventTelemetry {
    active: AtomicUsize,
    opened_total: AtomicU64,
    closed_total: AtomicU64,
    lagged_messages: AtomicU64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HostEventTelemetrySnapshot {
    pub active: usize,
    pub opened_total: u64,
    pub closed_total: u64,
    pub lagged_messages: u64,
}

impl HostEventTelemetry {
    pub fn stream_opened(&self) -> HostEventTelemetrySnapshot {
        self.opened_total.fetch_add(1, Ordering::Relaxed);
        self.active.fetch_add(1, Ordering::Relaxed);
        self.snapshot()
    }

    pub fn stream_closed(&self) -> HostEventTelemetrySnapshot {
        self.closed_total.fetch_add(1, Ordering::Relaxed);
        let _ = self
            .active
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |active| {
                Some(active.saturating_sub(1))
            });
        self.snapshot()
    }

    pub fn record_lagged(&self, skipped: u64) -> HostEventTelemetrySnapshot {
        self.lagged_messages.fetch_add(skipped, Ordering::Relaxed);
        self.snapshot()
    }

    pub fn snapshot(&self) -> HostEventTelemetrySnapshot {
        HostEventTelemetrySnapshot {
            active: self.active.load(Ordering::Relaxed),
            opened_total: self.opened_total.load(Ordering::Relaxed),
            closed_total: self.closed_total.load(Ordering::Relaxed),
            lagged_messages: self.lagged_messages.load(Ordering::Relaxed),
        }
    }
}

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
    /// None means the host is bound as a control plane without a current app.
    pub default_app_id: Option<String>,
    pub selected_profile_id: Option<String>,
    pub selected_profile_file: Option<String>,
    pub selected_profile_revision: Option<String>,
    pub selected_profile_source: Option<String>,
    /// True only after an explicit app startup or a successful profile apply.
    pub data_plane_enabled: bool,
    pub package_root: std::path::PathBuf,
    /// Default-app plug-ds endpoint (ops banner / backward compat).
    pub plug_ds_endpoint: String,
    pub plug_ds_by_app: BTreeMap<String, String>,
    pub plug_ds_managed: bool,
    /// instance_id → endpoint (mirrors supervisor pool).
    pub app_runtime_by_instance: BTreeMap<String, String>,
    /// instance_id → spawn/ready wall-clock ms (mirrors supervisor pool).
    pub app_runtime_started_at_ms: BTreeMap<String, u64>,
    /// In-memory LaunchManifest / route table view (control-plane + gateway).
    pub launch_manifest: LaunchManifest,
    /// True when at least one desired Running instance is reachable via supervisor.
    pub route_plane_ready: bool,
    pub imported: bool,
    pub warmed_up: bool,
    pub host_started_at_ms: u64,
    pub ops_job: Option<OpsJobState>,
    pub last_ops_job: Option<OpsJobState>,
    pub cleanup_preview: Option<CleanupPreviewState>,
    /// Bounded fan-out for host control SSE. Receivers may reconnect after lag/disconnect.
    pub events: broadcast::Sender<HostEvent>,
    /// Process-wide lifecycle counters for Host control SSE connections.
    pub event_telemetry: Arc<HostEventTelemetry>,
    /// `preparing` / `waiting_artifacts` / `importing` / `plug_ds` / `priming_cache` / `ready` / `failed`
    pub startup_phase: String,
    pub startup_detail: Option<String>,
    pub startup_error: Option<String>,
    pub app_materialization: BTreeMap<String, AppMaterializationState>,
    /// Process-level default max data capability (`eval` default). Prefer
    /// [`ShellState::data_mode_ceiling_for`] for app-scoped API checks.
    pub data_mode_ceiling: DataModeCeiling,
    /// Per-app ceiling from the last launch applied to that app. Prevents a
    /// static tutorial app from blocking eval APIs for hot data apps.
    pub data_mode_ceiling_by_app: BTreeMap<String, DataModeCeiling>,
}

impl ShellState {
    pub fn new(
        workspace: std::path::PathBuf,
        app_id: String,
        package_root: std::path::PathBuf,
        plug_ds_by_app: BTreeMap<String, String>,
        plug_ds_managed: bool,
    ) -> Self {
        let default_app_id = (!app_id.trim().is_empty()).then(|| app_id.clone());
        let plug_ds_endpoint = plug_ds_by_app
            .get(app_id.as_str())
            .cloned()
            .unwrap_or_default();
        Self {
            ctx: HostContext::new(workspace, app_id),
            default_app_id,
            selected_profile_id: None,
            selected_profile_file: None,
            selected_profile_revision: None,
            selected_profile_source: None,
            data_plane_enabled: false,
            package_root,
            plug_ds_endpoint,
            plug_ds_by_app,
            plug_ds_managed,
            app_runtime_by_instance: BTreeMap::new(),
            app_runtime_started_at_ms: BTreeMap::new(),
            launch_manifest: LaunchManifest::empty(),
            route_plane_ready: false,
            imported: false,
            warmed_up: false,
            host_started_at_ms: current_time_ms(),
            ops_job: None,
            last_ops_job: None,
            cleanup_preview: None,
            events: host_event_channel(),
            event_telemetry: Arc::new(HostEventTelemetry::default()),
            startup_phase: "preparing".to_string(),
            startup_detail: Some("正在启动 MeiLang 宿主服务…".to_string()),
            startup_error: None,
            app_materialization: BTreeMap::new(),
            data_mode_ceiling: DataModeCeiling::Eval,
            data_mode_ceiling_by_app: BTreeMap::new(),
        }
    }

    /// Ceiling for an app's datasets/bootstrap APIs. Falls back to process default.
    pub fn data_mode_ceiling_for(&self, app_id: &str) -> DataModeCeiling {
        self.data_mode_ceiling_by_app
            .get(app_id)
            .copied()
            .unwrap_or(self.data_mode_ceiling)
    }

    pub fn set_data_mode_ceiling_for(&mut self, app_id: &str, ceiling: DataModeCeiling) {
        let app_id = app_id.trim();
        if app_id.is_empty() {
            self.data_mode_ceiling = ceiling;
            return;
        }
        self.data_mode_ceiling_by_app
            .insert(app_id.to_string(), ceiling);
        if self.default_app() == Some(app_id) {
            self.data_mode_ceiling = ceiling;
        }
    }

    pub fn host_ctx_for_app(&self, app_id: &str) -> HostContext {
        HostContext::new(self.ctx.workspace_root.clone(), app_id.to_string())
    }

    pub fn set_default_app(&mut self, app_id: Option<String>) {
        self.default_app_id = app_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        self.ctx.app_id = self.default_app_id.clone().unwrap_or_default();
        self.plug_ds_endpoint = self
            .default_app_id
            .as_deref()
            .and_then(|app_id| self.plug_ds_by_app.get(app_id))
            .cloned()
            .unwrap_or_default();
    }

    pub fn default_app(&self) -> Option<&str> {
        self.default_app_id.as_deref()
    }

    pub fn plug_ds_endpoint_for(&self, app_id: &str) -> Option<&str> {
        self.plug_ds_by_app.get(app_id).map(String::as_str)
    }

    /// Resolve active route → instance → endpoint for an app.
    pub fn endpoint_for_app(&self, app_id: &str) -> Option<&str> {
        let instance_id = self
            .launch_manifest
            .routes
            .get(app_id)
            .and_then(|route: &RouteBinding| route.active.as_deref())?;
        self.app_runtime_by_instance
            .get(instance_id)
            .map(String::as_str)
    }

    pub fn active_instance_id_for_app(&self, app_id: &str) -> Option<&str> {
        self.launch_manifest
            .routes
            .get(app_id)
            .and_then(|route| route.active.as_deref())
    }

    pub fn install_launch_manifest(&mut self, manifest: LaunchManifest) {
        self.launch_manifest = manifest;
        self.refresh_route_plane_ready();
    }

    pub fn sync_app_runtime_endpoints(&mut self, endpoints: BTreeMap<String, String>) {
        self.sync_app_runtime_endpoints_with_started(endpoints, BTreeMap::new());
    }

    pub fn sync_app_runtime_endpoints_with_started(
        &mut self,
        endpoints: BTreeMap<String, String>,
        started_at: BTreeMap<String, u64>,
    ) {
        let mut next_started = BTreeMap::new();
        for id in endpoints.keys() {
            let ms = started_at
                .get(id)
                .copied()
                .or_else(|| self.app_runtime_started_at_ms.get(id).copied())
                .unwrap_or_else(current_time_ms);
            next_started.insert(id.clone(), ms);
        }
        self.app_runtime_by_instance = endpoints;
        self.app_runtime_started_at_ms = next_started;
        self.refresh_route_plane_ready();
    }

    pub fn register_app_runtime_endpoint(
        &mut self,
        instance_id: impl Into<String>,
        endpoint: impl Into<String>,
        started_at_ms: Option<u64>,
    ) {
        let instance_id = instance_id.into();
        self.app_runtime_by_instance
            .insert(instance_id.clone(), endpoint.into());
        self.app_runtime_started_at_ms
            .insert(instance_id, started_at_ms.unwrap_or_else(current_time_ms));
        self.refresh_route_plane_ready();
    }

    pub fn unregister_app_runtime_endpoint(&mut self, instance_id: &str) {
        self.app_runtime_by_instance.remove(instance_id);
        self.app_runtime_started_at_ms.remove(instance_id);
        self.refresh_route_plane_ready();
    }

    pub fn refresh_route_plane_ready(&mut self) {
        self.route_plane_ready = self.launch_manifest.routes.values().any(|route| {
            route
                .active
                .as_ref()
                .is_some_and(|id| self.app_runtime_by_instance.contains_key(id.as_str()))
        });
    }

    /// Control-plane readiness is independent of Access / route plane readiness.
    pub fn control_plane_ready(&self) -> bool {
        true
    }

    pub fn access_route_ready_for(&self, app_id: &str) -> bool {
        self.endpoint_for_app(app_id).is_some()
            || (self.data_plane_enabled && self.imported && self.default_app() == Some(app_id))
    }
}

pub type SharedState = Arc<RwLock<ShellState>>;

#[derive(Clone)]
pub struct HostHttpState {
    pub shell: SharedState,
    pub auth: AuthServeState,
    pub managed_plug: Arc<Mutex<Option<ManagedPlugDsPool>>>,
    /// Resident app-runtime supervisor (never taken as `None` during spawn).
    pub app_runtime: SharedAppRuntime,
    /// Proxy/readiness read snapshot — updated on start/stop/cutover.
    pub route_table: SharedRuntimeRouteTable,
    /// Per-app start dedupe for UI double-clicks.
    pub start_inflight: Arc<AppStartInflight>,
    /// Serializes short LaunchManifest CAS mutations across independent app lanes.
    pub manifest_mutation: Arc<tokio::sync::Mutex<()>>,
    /// Optional control-plane actor mailbox (serializes Start/Stop).
    pub runtime_actor: Option<RuntimeActorHandle>,
}

impl HostHttpState {
    pub fn with_defaults(
        shell: SharedState,
        auth: AuthServeState,
        managed_plug: Arc<Mutex<Option<ManagedPlugDsPool>>>,
        app_runtime: SharedAppRuntime,
    ) -> Self {
        Self {
            shell,
            auth,
            managed_plug,
            app_runtime,
            route_table: SharedRuntimeRouteTable::new(),
            start_inflight: Arc::new(AppStartInflight::default()),
            manifest_mutation: Arc::new(tokio::sync::Mutex::new(())),
            runtime_actor: None,
        }
    }

    pub fn publish_route_running(
        &self,
        app_id: &str,
        instance_id: &str,
        endpoint: &str,
        token: &str,
        generation: &str,
        spec_digest: &str,
    ) {
        self.route_table.publish_entry(RuntimeRouteEntry {
            app_id: app_id.to_string(),
            instance_id: instance_id.to_string(),
            endpoint: endpoint.to_string(),
            token: token.to_string(),
            generation: generation.to_string(),
            spec_digest: spec_digest.to_string(),
            phase: RuntimeRoutePhase::Running,
        });
    }

    pub fn mark_route_draining(&self, app_id: &str) {
        self.route_table
            .set_phase(app_id, RuntimeRoutePhase::Draining);
    }

    pub fn remove_route(&self, app_id: &str) {
        self.route_table.remove_app(app_id);
    }

    pub async fn sync_route_table_from_supervisor(&self) {
        let (endpoints, tokens, generations, digests) = {
            let guard = self.app_runtime.lock().await;
            (
                guard.endpoint_map(),
                guard.token_map(),
                guard.generation_map(),
                guard.digest_map(),
            )
        };
        let routes = {
            let shell = self.shell.read().expect("state lock");
            shell.launch_manifest.routes.clone()
        };
        self.route_table
            .rebuild_from_shell(&routes, &endpoints, &tokens, &generations, &digests);
    }
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

pub fn runtime_identity_from_snapshot(
    route_table: &SharedRuntimeRouteTable,
    app_id: &str,
    principal: Option<AuthPrincipal>,
) -> Option<RuntimeProxyIdentity> {
    route_table.load().identity_for(app_id, principal)
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
    use mei_host_core::{DesiredInstance, DesiredState, RouteBinding};
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn host_event_telemetry_tracks_lifecycle_and_lag() {
        let telemetry = HostEventTelemetry::default();
        assert_eq!(
            telemetry.snapshot(),
            HostEventTelemetrySnapshot {
                active: 0,
                opened_total: 0,
                closed_total: 0,
                lagged_messages: 0,
            }
        );
        assert_eq!(telemetry.stream_opened().active, 1);
        assert_eq!(telemetry.record_lagged(7).lagged_messages, 7);
        let closed = telemetry.stream_closed();
        assert_eq!(closed.active, 0);
        assert_eq!(closed.opened_total, 1);
        assert_eq!(closed.closed_total, 1);
    }

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

    #[test]
    fn endpoint_for_app_resolves_active_route() {
        let mut state = ShellState::new(
            Path::new("/tmp/ws").to_path_buf(),
            "mini-data".to_string(),
            Path::new("/tmp/pkg").to_path_buf(),
            BTreeMap::new(),
            false,
        );
        let mut manifest = LaunchManifest::empty();
        manifest.instances.insert(
            "inst-1".to_string(),
            DesiredInstance {
                spec_ref: "sha256:a".to_string(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.routes.insert(
            "mini-data".to_string(),
            RouteBinding {
                active: Some("inst-1".to_string()),
                candidate: None,
                previous: None,
            },
        );
        state.install_launch_manifest(manifest.with_recomputed_revision());
        state
            .app_runtime_by_instance
            .insert("inst-1".to_string(), "http://127.0.0.1:7777".to_string());
        state.refresh_route_plane_ready();
        assert_eq!(
            state.endpoint_for_app("mini-data"),
            Some("http://127.0.0.1:7777")
        );
        assert!(state.route_plane_ready);
        assert_eq!(state.endpoint_for_app("other"), None);
    }
}
