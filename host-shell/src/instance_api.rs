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
    use mei_host_core::{DesiredInstance, DesiredState, RouteBinding};

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
}
