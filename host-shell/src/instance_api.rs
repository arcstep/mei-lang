//! Host Control Center APIs for LaunchManifest and ObservedInstance.

use std::collections::{BTreeMap, BTreeSet};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{
    DesiredState, InstanceHealth, InstancePhase, InstanceResource, InstanceRevisions,
    LaunchManifest, ObservedInstance,
};
use serde::Serialize;
use serde_json::json;

use crate::app_runtime_supervisor::AppRuntimeSupervisor;
use crate::state::{HostEvent, HostHttpState, SharedState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListItem {
    #[serde(flatten)]
    pub observed: ObservedInstance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_role: Option<String>,
}

pub async fn api_host_launch_manifest(State(http): State<HostHttpState>) -> Response {
    let guard = http.shell.read().expect("state lock");
    let manifest = guard.launch_manifest.clone();
    Json(json!({
        "revision": manifest.revision,
        "manifest": manifest,
    }))
    .into_response()
}

pub async fn api_host_instances(State(http): State<HostHttpState>) -> Response {
    let (manifest, shell_endpoints) = {
        let guard = http.shell.read().expect("state lock");
        (
            guard.launch_manifest.clone(),
            guard.app_runtime_by_instance.clone(),
        )
    };
    let supervisor = http.app_runtime.lock().await;
    let items = collect_observed_instances(&manifest, Some(&*supervisor), &shell_endpoints);
    Json(json!({
        "revision": manifest.revision,
        "instances": items,
        "routes": manifest.routes,
    }))
    .into_response()
}

pub async fn api_host_instance_stop(
    State(http): State<HostHttpState>,
    AxumPath(instance_id): AxumPath<String>,
) -> Response {
    match stop_instance(&http, instance_id.as_str()).await {
        Ok(observed) => {
            emit_instance_event(&http.shell, "instance-phase", &observed, Some("stopped"));
            Json(json!({
                "accepted": true,
                "kind": "instance-stop",
                "instance": observed,
            }))
            .into_response()
        }
        Err(error) => instance_error(error),
    }
}

pub async fn api_host_instance_restart(
    State(http): State<HostHttpState>,
    AxumPath(instance_id): AxumPath<String>,
) -> Response {
    match restart_instance(&http, instance_id.as_str()).await {
        Ok(observed) => {
            let event = match observed.phase {
                InstancePhase::Ready => "instance-ready",
                InstancePhase::Failed => "instance-failed",
                _ => "instance-phase",
            };
            emit_instance_event(&http.shell, event, &observed, None);
            Json(json!({
                "accepted": true,
                "kind": "instance-restart",
                "instance": observed,
            }))
            .into_response()
        }
        Err(error) => instance_error(error),
    }
}

fn collect_observed_instances(
    manifest: &LaunchManifest,
    supervisor: Option<&AppRuntimeSupervisor>,
    shell_endpoints: &BTreeMap<String, String>,
) -> Vec<InstanceListItem> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();

    for (instance_id, desired) in &manifest.instances {
        seen.insert(instance_id.clone());
        let (app_id, route_role) = route_context_for(manifest, instance_id.as_str());
        let observed = if let Some(managed) = supervisor.and_then(|s| s.runtime_for(instance_id)) {
            let mut observed = observed_from_runtime(managed, desired.desired_state, None);
            observed.protected_reasons = protected_reasons(route_role.as_deref());
            observed
        } else if let Some(endpoint) = shell_endpoints.get(instance_id.as_str()) {
            ObservedInstance {
                instance_id: instance_id.clone(),
                spec_ref: desired.spec_ref.clone(),
                observed_at_ms: crate::state::current_time_ms(),
                phase: InstancePhase::Ready,
                desired_state: desired.desired_state,
                reachable: true,
                endpoint: Some(endpoint.clone()),
                token_present: false,
                health: InstanceHealth {
                    process: "ok".to_string(),
                    plug_ds: "unknown".to_string(),
                    warmup: "unknown".to_string(),
                    bootstrap: "unknown".to_string(),
                },
                revisions: InstanceRevisions::default(),
                protected_reasons: protected_reasons(route_role.as_deref()),
                last_error: None,
                resource: InstanceResource::default(),
            }
        } else {
            ObservedInstance {
                instance_id: instance_id.clone(),
                spec_ref: desired.spec_ref.clone(),
                observed_at_ms: crate::state::current_time_ms(),
                phase: match desired.desired_state {
                    DesiredState::Stopped => InstancePhase::Stopped,
                    DesiredState::Standby => InstancePhase::Stopped,
                    DesiredState::Running => InstancePhase::Queued,
                },
                desired_state: desired.desired_state,
                reachable: false,
                endpoint: None,
                token_present: false,
                health: InstanceHealth {
                    process: "stopped".to_string(),
                    plug_ds: "unknown".to_string(),
                    warmup: "unknown".to_string(),
                    bootstrap: "unknown".to_string(),
                },
                revisions: InstanceRevisions::default(),
                protected_reasons: protected_reasons(route_role.as_deref()),
                last_error: None,
                resource: InstanceResource::default(),
            }
        };
        items.push(InstanceListItem {
            observed,
            app_id,
            route_role,
        });
    }

    if let Some(supervisor) = supervisor {
        for (instance_id, managed) in &supervisor.runtimes {
            if seen.contains(instance_id.as_str()) {
                continue;
            }
            let (app_id, route_role) = route_context_for(manifest, instance_id.as_str());
            let app_id = app_id.or_else(|| Some(managed.spec.app_id.clone()));
            items.push(InstanceListItem {
                observed: observed_from_runtime(managed, DesiredState::Running, None),
                app_id,
                route_role,
            });
        }
    }

    items.sort_by(|left, right| left.observed.instance_id.cmp(&right.observed.instance_id));
    items
}

fn route_context_for(
    manifest: &LaunchManifest,
    instance_id: &str,
) -> (Option<String>, Option<String>) {
    for (app_id, route) in &manifest.routes {
        if route.active.as_deref() == Some(instance_id) {
            return (Some(app_id.clone()), Some("active".to_string()));
        }
        if route.candidate.as_deref() == Some(instance_id) {
            return (Some(app_id.clone()), Some("candidate".to_string()));
        }
        if route.previous.as_deref() == Some(instance_id) {
            return (Some(app_id.clone()), Some("previous".to_string()));
        }
    }
    (None, None)
}

fn protected_reasons(route_role: Option<&str>) -> Vec<String> {
    match route_role {
        Some("active") => vec!["active-route".to_string()],
        Some("candidate") => vec!["candidate-route".to_string()],
        Some("previous") => vec!["previous-route".to_string()],
        _ => Vec::new(),
    }
}

fn observed_from_runtime(
    managed: &crate::app_runtime_supervisor::ManagedRuntime,
    desired_state: DesiredState,
    last_error: Option<String>,
) -> ObservedInstance {
    ObservedInstance {
        instance_id: managed.spec.instance_id.clone(),
        spec_ref: managed.spec.spec_digest(),
        observed_at_ms: crate::state::current_time_ms(),
        phase: if last_error.is_some() {
            InstancePhase::Failed
        } else {
            InstancePhase::Ready
        },
        desired_state,
        reachable: last_error.is_none(),
        endpoint: Some(managed.endpoint.clone()),
        token_present: !managed.token.is_empty(),
        health: InstanceHealth {
            process: if last_error.is_none() {
                "ok".to_string()
            } else {
                "failed".to_string()
            },
            plug_ds: "ok".to_string(),
            warmup: "ready".to_string(),
            bootstrap: "ok".to_string(),
        },
        revisions: InstanceRevisions {
            data_generation: Some(managed.spec.bundle.generation.clone()),
            ..InstanceRevisions::default()
        },
        protected_reasons: Vec::new(),
        last_error,
        resource: InstanceResource {
            generation: Some(managed.spec.bundle.generation.clone()),
            ..InstanceResource::default()
        },
    }
}

async fn stop_instance(
    http: &HostHttpState,
    instance_id: &str,
) -> Result<ObservedInstance, InstanceApiError> {
    if instance_id.trim().is_empty() {
        return Err(InstanceApiError::BadRequest(
            "instance id must not be empty".into(),
        ));
    }
    {
        let guard = http.shell.read().expect("state lock");
        if let Some(route) = active_route_for(&guard.launch_manifest, instance_id) {
            return Err(InstanceApiError::Conflict(format!(
                "cannot stop active route instance `{instance_id}` for app `{route}`"
            )));
        }
    }
    let _ = crate::app_runtime_supervisor::stop_from(&http.app_runtime, instance_id)
        .await
        .map_err(|e| InstanceApiError::Other(e.to_string()))?;
    let (endpoints, started_at) = {
        let guard = http.app_runtime.lock().await;
        (guard.endpoint_map(), guard.started_at_map())
    };
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    http.sync_route_table_from_supervisor().await;
    Ok(ObservedInstance {
        instance_id: instance_id.to_string(),
        spec_ref: String::new(),
        observed_at_ms: crate::state::current_time_ms(),
        phase: InstancePhase::Stopped,
        desired_state: DesiredState::Stopped,
        reachable: false,
        endpoint: None,
        token_present: false,
        health: InstanceHealth {
            process: "stopped".to_string(),
            plug_ds: "unknown".to_string(),
            warmup: "unknown".to_string(),
            bootstrap: "unknown".to_string(),
        },
        revisions: InstanceRevisions::default(),
        protected_reasons: Vec::new(),
        last_error: None,
        resource: InstanceResource::default(),
    })
}

async fn restart_instance(
    http: &HostHttpState,
    instance_id: &str,
) -> Result<ObservedInstance, InstanceApiError> {
    if instance_id.trim().is_empty() {
        return Err(InstanceApiError::BadRequest(
            "instance id must not be empty".into(),
        ));
    }
    emit_instance_id_event(
        &http.shell,
        "instance-phase",
        instance_id,
        InstancePhase::Launching,
        None,
    );
    let result = crate::app_runtime_supervisor::restart_from(&http.app_runtime, instance_id, 3)
        .await
        .map_err(|e| {
            if e.to_string().contains("is not managed") {
                InstanceApiError::NotFound(format!("instance `{instance_id}` is not managed"))
            } else {
                InstanceApiError::Other(e.to_string())
            }
        })?;
    let (endpoints, started_at) = {
        let guard = http.app_runtime.lock().await;
        (guard.endpoint_map(), guard.started_at_map())
    };
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    http.sync_route_table_from_supervisor().await;
    Ok(result)
}

/// When a managed app-runtime child exits (e.g. stack overflow abort), restart it
/// and republish routes so Access does not stay wedged on a dead endpoint.
///
/// Survives brief map gaps during intentional `restart_from` (stop then spawn).
/// Call once after a successful spawn; do not re-arm from `restart_instance`.
///
/// Poll interval: `MEI_RUNTIME_WATCHDOG_POLL_MS` (default 1000). Tests may set a
/// smaller value so kill→recover closes quickly.
pub fn arm_runtime_exit_watchdog(http: HostHttpState, instance_id: String) {
    if instance_id.trim().is_empty() {
        return;
    }
    let poll = watchdog_poll_interval();
    tokio::spawn(async move {
        let mut consecutive_missing = 0u32;
        loop {
            tokio::time::sleep(poll).await;
            let exited = {
                let mut guard = http.app_runtime.lock().await;
                let Some(managed) = guard.runtimes.get_mut(instance_id.as_str()) else {
                    consecutive_missing = consecutive_missing.saturating_add(1);
                    // Intentional stop removes the entry; mid-restart leaves a short gap.
                    if consecutive_missing > 60 {
                        tracing::info!(
                            instance_id = %instance_id,
                            "app-runtime exit watchdog stopping (instance absent)"
                        );
                        return;
                    }
                    continue;
                };
                consecutive_missing = 0;
                match managed.child.try_wait() {
                    Ok(Some(status)) => Some((
                        status.to_string(),
                        managed.child_pid,
                        managed.endpoint.clone(),
                        managed.spec.app_id.clone(),
                    )),
                    Ok(None) => None,
                    Err(error) => Some((
                        format!("try_wait error: {error}"),
                        managed.child_pid,
                        managed.endpoint.clone(),
                        managed.spec.app_id.clone(),
                    )),
                }
            };
            let Some((status, child_pid, dead_endpoint, app_id)) = exited else {
                continue;
            };
            // Serious accident record: Host event bus + error log (must not be silent).
            emit_runtime_accident(
                &http.shell,
                instance_id.as_str(),
                app_id.as_str(),
                child_pid,
                dead_endpoint.as_str(),
                status.as_str(),
            );
            tracing::error!(
                instance_id = %instance_id,
                app_id = %app_id,
                child_pid = ?child_pid,
                endpoint = %dead_endpoint,
                status = %status,
                severity = "critical",
                "runtime-accident: app-runtime exited unexpectedly; attempting automatic restart"
            );
            match restart_instance(&http, instance_id.as_str()).await {
                Ok(observed) => {
                    emit_runtime_recovered(&http.shell, instance_id.as_str(), &observed);
                    tracing::warn!(
                        instance_id = %instance_id,
                        endpoint = ?observed.endpoint,
                        "app-runtime auto-restart succeeded after runtime-accident"
                    );
                    // Keep watching the replacement child (same instance_id).
                }
                Err(error) => {
                    tracing::error!(
                        instance_id = %instance_id,
                        error = ?error,
                        severity = "critical",
                        "runtime-accident: app-runtime auto-restart failed"
                    );
                    emit_instance_id_event(
                        &http.shell,
                        "runtime-accident-unrecovered",
                        instance_id.as_str(),
                        InstancePhase::Failed,
                        Some(&format!("{error:?}")),
                    );
                    return;
                }
            }
        }
    });
}

fn watchdog_poll_interval() -> std::time::Duration {
    let ms = std::env::var("MEI_RUNTIME_WATCHDOG_POLL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1000);
    std::time::Duration::from_millis(ms)
}

fn emit_runtime_accident(
    shell: &SharedState,
    instance_id: &str,
    app_id: &str,
    child_pid: Option<u32>,
    endpoint: &str,
    status: &str,
) {
    let guard = shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        "runtime-accident",
        json!({
            "severity": "critical",
            "kind": "app_runtime_child_exited",
            "instanceId": instance_id,
            "appId": app_id,
            "childPid": child_pid,
            "endpoint": endpoint,
            "status": status,
            "action": "auto_restart",
        }),
    ));
}

fn emit_runtime_recovered(
    shell: &SharedState,
    instance_id: &str,
    observed: &ObservedInstance,
) {
    let guard = shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        "runtime-recovered",
        json!({
            "severity": "info",
            "kind": "app_runtime_auto_restart",
            "instanceId": instance_id,
            "endpoint": observed.endpoint,
            "reachable": observed.reachable,
            "generation": observed.resource.generation,
        }),
    ));
}

fn active_route_for(manifest: &LaunchManifest, instance_id: &str) -> Option<String> {
    manifest.routes.iter().find_map(|(app_id, route)| {
        (route.active.as_deref() == Some(instance_id)).then(|| app_id.clone())
    })
}

pub fn emit_instance_event(
    shell: &SharedState,
    event_type: &str,
    observed: &ObservedInstance,
    phase_override: Option<&str>,
) {
    let phase = phase_override
        .map(str::to_string)
        .unwrap_or_else(|| format!("{:?}", observed.phase).to_ascii_lowercase());
    let guard = shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        event_type,
        json!({
            "instanceId": observed.instance_id,
            "phase": phase,
            "endpoint": observed.endpoint,
            "reachable": observed.reachable,
            "generation": observed.resource.generation,
            "lastError": observed.last_error,
        }),
    ));
}

pub fn emit_instance_id_event(
    shell: &SharedState,
    event_type: &str,
    instance_id: &str,
    phase: InstancePhase,
    last_error: Option<&str>,
) {
    let guard = shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        event_type,
        json!({
            "instanceId": instance_id,
            "phase": format!("{phase:?}").to_ascii_lowercase(),
            "lastError": last_error,
        }),
    ));
}

pub fn emit_builder_phase(shell: &SharedState, phase: &str, message: impl Into<String>) {
    let guard = shell.read().expect("state lock");
    let job = guard.ops_job.clone();
    let _ = guard.events.send(HostEvent::new(
        "builder-phase",
        json!({
            "phase": phase,
            "message": message.into(),
            "job": job,
        }),
    ));
}

#[derive(Debug)]
enum InstanceApiError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Other(String),
}

fn instance_error(error: InstanceApiError) -> Response {
    let (status, code, message) = match error {
        InstanceApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
        InstanceApiError::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
        InstanceApiError::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
        InstanceApiError::Other(message) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "instance_error", message)
        }
    };
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, RwLock};

    use mei_host_core::{DesiredInstance, DesiredState, HostContext, RouteBinding};

    use crate::app_runtime_supervisor::{
        generate_instance_token, spawn_into, synthesize_instance_spec,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn collect_marks_route_roles() {
        let mut manifest = LaunchManifest::empty();
        manifest.instances.insert(
            "inst-a".into(),
            DesiredInstance {
                spec_ref: "sha:a".into(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.instances.insert(
            "inst-b".into(),
            DesiredInstance {
                spec_ref: "sha:b".into(),
                desired_state: DesiredState::Standby,
            },
        );
        manifest.routes.insert(
            "mini-data".into(),
            RouteBinding {
                active: Some("inst-a".into()),
                candidate: None,
                previous: Some("inst-b".into()),
            },
        );
        let items = collect_observed_instances(&manifest, None, &BTreeMap::new());
        assert_eq!(items.len(), 2);
        let active = items
            .iter()
            .find(|item| item.observed.instance_id == "inst-a")
            .expect("active");
        assert_eq!(active.route_role.as_deref(), Some("active"));
        assert_eq!(active.app_id.as_deref(), Some("mini-data"));
        let previous = items
            .iter()
            .find(|item| item.observed.instance_id == "inst-b")
            .expect("previous");
        assert_eq!(previous.route_role.as_deref(), Some("previous"));
    }

    fn write_fake_app_runtime(bin_path: &std::path::Path) {
        let script = r#"#!/usr/bin/env python3
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port = 0
args = sys.argv[1:]
for i, a in enumerate(args):
    if a == "--port" and i + 1 < len(args):
        port = int(args[i + 1])
        break
if port <= 0:
    sys.stderr.write("fake runtime: missing --port\n")
    sys.exit(2)
print(f"MEI_APP_RUNTIME_LISTEN=http://127.0.0.1:{port}", flush=True)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/api/app-runtime/ready"):
            body = json.dumps({"ready": True, "phase": "ready"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, *_args):
        return

HTTPServer(("127.0.0.1", port), Handler).serve_forever()
"#;
        std::fs::write(bin_path, script).expect("write fake runtime");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(bin_path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(bin_path, perms).expect("chmod");
        }
    }

    fn test_http_state(workspace: std::path::PathBuf) -> HostHttpState {
        let shell = Arc::new(RwLock::new(crate::state::ShellState {
            ctx: HostContext::new(workspace.clone(), "demo".to_string()),
            default_app_id: Some("demo".to_string()),
            selected_profile_id: Some("default".to_string()),
            selected_profile_file: Some("workspace.json".to_string()),
            selected_profile_revision: Some("test".to_string()),
            selected_profile_source: Some("workspace_default".to_string()),
            data_plane_enabled: true,
            package_root: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            plug_ds_endpoint: String::new(),
            plug_ds_by_app: BTreeMap::new(),
            plug_ds_managed: false,
            app_runtime_by_instance: BTreeMap::new(),
            app_runtime_started_at_ms: BTreeMap::new(),
            launch_manifest: LaunchManifest::empty(),
            enabled_apps: BTreeSet::new(),
            route_plane_ready: false,
            imported: true,
            warmed_up: true,
            host_started_at_ms: 1,
            ops_job: None,
            last_ops_job: None,
            cleanup_preview: None,
            events: crate::state::host_event_channel(),
            event_telemetry: Arc::new(crate::state::HostEventTelemetry::default()),
            startup_phase: "ready".to_string(),
            startup_detail: None,
            startup_error: None,
            app_materialization: BTreeMap::new(),
            data_mode_ceiling: mei_lang_kernel::DataModeCeiling::Eval,
            data_mode_ceiling_by_app: BTreeMap::new(),
            admin_registry: crate::admin_registry::AdminRegistry::shared(),
        }));
        HostHttpState::with_defaults(
            shell,
            mei_host_auth::AuthServeState::new(
                workspace.clone(),
                mei_host_auth::AuthEnforcement::Disabled,
            ),
            Arc::new(Mutex::new(None)),
            crate::app_runtime_supervisor::empty_shared_app_runtime(workspace),
        )
    }

    /// TDD: kill managed child → Host records critical `runtime-accident` and rebuilds.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn watchdog_rebuilds_after_manual_kill_and_records_accident() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("apps/demo/env/current")).expect("app dir");

        let fake_bin = tmp.path().join("fake-mei-app-runtime");
        write_fake_app_runtime(&fake_bin);
        std::env::set_var("MEI_APP_RUNTIME_BIN", &fake_bin);
        std::env::set_var("MEI_RUNTIME_WATCHDOG_POLL_MS", "50");

        let http = test_http_state(workspace.clone());
        let mut events = {
            let guard = http.shell.read().expect("state lock");
            guard.events.subscribe()
        };

        let instance_id = "demo@test-watchdog".to_string();
        let spec = synthesize_instance_spec(workspace.as_path(), "demo", instance_id.as_str());
        let token = generate_instance_token(instance_id.as_str());
        let observed = spawn_into(&http.app_runtime, spec, token)
            .await
            .expect("spawn fake runtime");
        let old_pid = {
            let guard = http.app_runtime.lock().await;
            guard
                .runtime_for(instance_id.as_str())
                .and_then(|rt| rt.child_pid)
                .expect("old pid")
        };
        let old_endpoint = observed.endpoint.clone().expect("endpoint");

        arm_runtime_exit_watchdog(http.clone(), instance_id.clone());

        // Manual crash: SIGKILL the child (same class as abort / stack overflow death).
        #[cfg(unix)]
        {
            let kill_status = std::process::Command::new("kill")
                .args(["-9", &old_pid.to_string()])
                .status()
                .expect("kill");
            assert!(kill_status.success(), "kill -9 {old_pid} failed");
        }
        #[cfg(not(unix))]
        {
            let _ = old_pid;
            panic!("watchdog kill TDD requires unix");
        }

        let mut saw_accident = false;
        let mut saw_recovered = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(200), events.recv()).await
            {
                Ok(Ok(ev)) if ev.event_type == "runtime-accident" => {
                    assert_eq!(
                        ev.payload.get("severity").and_then(|v| v.as_str()),
                        Some("critical")
                    );
                    assert_eq!(
                        ev.payload.get("kind").and_then(|v| v.as_str()),
                        Some("app_runtime_child_exited")
                    );
                    assert_eq!(
                        ev.payload.get("instanceId").and_then(|v| v.as_str()),
                        Some(instance_id.as_str())
                    );
                    saw_accident = true;
                }
                Ok(Ok(ev)) if ev.event_type == "runtime-recovered" => {
                    saw_recovered = true;
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => {}
            }
            if saw_accident && saw_recovered {
                break;
            }
            let alive = {
                let mut guard = http.app_runtime.lock().await;
                match guard.runtimes.get_mut(instance_id.as_str()) {
                    Some(rt) => matches!(rt.child.try_wait(), Ok(None)),
                    None => false,
                }
            };
            if saw_accident && alive {
                // recovered process is running; wait briefly for recovered event
                if saw_recovered {
                    break;
                }
            }
        }

        assert!(
            saw_accident,
            "expected critical runtime-accident HostEvent after kill"
        );
        assert!(
            saw_recovered,
            "expected runtime-recovered HostEvent after auto-restart"
        );

        let (new_pid, new_endpoint) = {
            let mut guard = http.app_runtime.lock().await;
            let rt = guard
                .runtimes
                .get_mut(instance_id.as_str())
                .expect("instance still managed after restart");
            assert!(
                matches!(rt.child.try_wait(), Ok(None)),
                "replacement child must be alive"
            );
            (rt.child_pid.expect("new pid"), rt.endpoint.clone())
        };
        assert_ne!(new_pid, old_pid, "child pid must change after rebuild");
        assert!(
            !new_endpoint.is_empty(),
            "replacement must publish endpoint"
        );
        // Endpoint may reuse port in theory; pid change is the hard proof of rebuild.
        let _ = (old_endpoint, new_endpoint);

        // Cleanup: stop without leaving watchdog noise.
        let _ = crate::app_runtime_supervisor::stop_from(&http.app_runtime, instance_id.as_str())
            .await;
        std::env::remove_var("MEI_APP_RUNTIME_BIN");
        std::env::remove_var("MEI_RUNTIME_WATCHDOG_POLL_MS");
    }
}

