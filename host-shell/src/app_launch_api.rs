//! HTTP APIs for per-app launch configs and start/stop (0537).

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{
    ensure_default_launch_config, list_launch_configs, read_launch_config, write_launch_config,
    AppLaunchConfig, DesiredInstance, DesiredState, LaunchManifest, RouteBinding,
};
use serde::Deserialize;
use serde_json::json;

use crate::app_runtime_supervisor::{
    generate_instance_token, instance_spec_from_launch, AppRuntimeSupervisor,
};
use crate::state::{HostEvent, HostHttpState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAppBody {
    /// Launch config name or relative path under the app.
    pub config: Option<String>,
}

pub async fn api_host_apps_overview(State(http): State<HostHttpState>) -> Response {
    Json(crate::shell_chrome::build_apps_overview_payload(&http)).into_response()
}

pub async fn api_host_app_launch_configs(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match list_launch_configs(workspace.as_path(), app_id.as_str()) {
        Ok(launches) => Json(json!({ "appId": app_id, "launches": launches })).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn api_host_app_start(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<StartAppBody>,
) -> Response {
    match start_app_with_launch(&http, app_id.as_str(), body.config.as_deref()).await {
        Ok(payload) => Json(payload).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_host_app_stop(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    match stop_app_runtime(&http, app_id.as_str()).await {
        Ok(payload) => Json(payload).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_host_app_ensure_default_launch(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match ensure_default_launch_config(workspace.as_path(), app_id.as_str()) {
        Ok(doc) => Json(json!({ "accepted": true, "launch": doc })).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLaunchBody {
    pub config: AppLaunchConfig,
}

pub async fn api_host_app_save_launch(
    State(http): State<HostHttpState>,
    AxumPath((app_id, name)): AxumPath<(String, String)>,
    Json(body): Json<SaveLaunchBody>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match write_launch_config(
        workspace.as_path(),
        app_id.as_str(),
        name.as_str(),
        &body.config,
    ) {
        Ok(doc) => Json(json!({ "accepted": true, "launch": doc })).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn start_app_with_launch(
    http: &HostHttpState,
    app_id: &str,
    config: Option<&str>,
) -> Result<serde_json::Value, StartStopError> {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    // Stop existing running process for this app first (single-process rule).
    let _ = stop_app_runtime(http, app_id).await;

    let config_ref = config.unwrap_or("default");
    let launch = match read_launch_config(workspace.as_path(), app_id, config_ref) {
        Ok(doc) => doc,
        Err(_) if config_ref == "default" => {
            ensure_default_launch_config(workspace.as_path(), app_id)
                .map_err(|e| StartStopError::BadRequest(e.to_string()))?
        }
        Err(error) => return Err(StartStopError::BadRequest(error.to_string())),
    };
    {
        let guard = http.shell.read().expect("state lock");
        let _ = guard.events.send(HostEvent::new(
            "app-starting",
            json!({
                "appId": app_id,
                "launchId": launch.id,
                "phase": "spawning",
                "message": "正在拉起 mei-app-runtime…",
            }),
        ));
    }
    let spec = instance_spec_from_launch(workspace.as_path(), app_id, &launch)
        .map_err(|e| StartStopError::BadRequest(e.to_string()))?;
    let token = generate_instance_token(spec.instance_id.as_str());
    let spec_for_state = spec.clone();

    {
        let mut supervisor_guard = http
            .app_runtime
            .lock()
            .map_err(|_| StartStopError::Conflict("app-runtime supervisor lock poisoned".into()))?;
        if supervisor_guard.is_none() {
            *supervisor_guard = Some(AppRuntimeSupervisor::new(workspace.clone()));
        }
    }

    let mut stolen = http
        .app_runtime
        .lock()
        .map_err(|_| StartStopError::Conflict("app-runtime supervisor lock poisoned".into()))?
        .take()
        .ok_or_else(|| StartStopError::Conflict("app-runtime supervisor missing".into()))?;
    let result = stolen.spawn_instance(spec, token).await;
    if let Ok(mut slot) = http.app_runtime.lock() {
        *slot = Some(stolen);
    }
    let observed = result.map_err(|e| StartStopError::Unavailable(e.to_string()))?;

    let _ = mei_host_core::write_instance_spec(workspace.as_path(), &spec_for_state);

    {
        let mut guard = http.shell.write().expect("state lock");
        guard.register_app_runtime_endpoint(
            spec_for_state.instance_id.clone(),
            observed.endpoint.clone().unwrap_or_default(),
            Some(crate::state::current_time_ms()),
        );
        let mut manifest = guard.launch_manifest.clone();
        manifest.instances.insert(
            spec_for_state.instance_id.clone(),
            DesiredInstance {
                spec_ref: spec_for_state.spec_digest(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.routes.insert(
            app_id.to_string(),
            RouteBinding {
                active: Some(spec_for_state.instance_id.clone()),
                candidate: None,
                previous: None,
            },
        );
        guard.launch_manifest = manifest.with_recomputed_revision();
        guard.route_plane_ready = true;
        guard.data_plane_enabled = true;
        let _ = guard.events.send(HostEvent::new(
            "app-started",
            crate::shell_chrome::running_event_payload(
                workspace.as_path(),
                app_id,
                launch.id.as_str(),
                spec_for_state.instance_id.as_str(),
            ),
        ));
    }

    Ok(json!({
        "accepted": true,
        "kind": "app-started",
        "appId": app_id,
        "launch": launch,
        "instance": observed,
    }))
}

pub async fn stop_app_runtime(
    http: &HostHttpState,
    app_id: &str,
) -> Result<serde_json::Value, StartStopError> {
    let active_id = {
        let guard = http.shell.read().expect("state lock");
        guard
            .launch_manifest
            .routes
            .get(app_id)
            .and_then(|r| r.active.clone())
    };
    let Some(instance_id) = active_id else {
        return Ok(json!({
            "accepted": true,
            "kind": "app-stopped",
            "appId": app_id,
            "alreadyStopped": true,
        }));
    };

    let mut stolen = http
        .app_runtime
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(supervisor) = stolen.as_mut() {
        let _ = supervisor.stop_instance(instance_id.as_str()).await;
    }
    if let Some(supervisor) = stolen {
        if let Ok(mut slot) = http.app_runtime.lock() {
            *slot = Some(supervisor);
        }
    }

    {
        let mut guard = http.shell.write().expect("state lock");
        guard.unregister_app_runtime_endpoint(instance_id.as_str());
        let mut manifest = guard.launch_manifest.clone();
        if let Some(route) = manifest.routes.get_mut(app_id) {
            route.previous = route.active.take();
            route.candidate = None;
        }
        if let Some(desired) = manifest.instances.get_mut(instance_id.as_str()) {
            desired.desired_state = DesiredState::Stopped;
        }
        guard.launch_manifest = manifest.with_recomputed_revision();
        let _ = guard.events.send(HostEvent::new(
            "app-stopped",
            json!({
                "appId": app_id,
                "instanceId": instance_id,
                "href": crate::shell_chrome::app_access_href(app_id),
            }),
        ));
    }

    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let _ = mei_host_core::clear_app_ephemeral_runtime(workspace.as_path(), app_id);

    Ok(json!({
        "accepted": true,
        "kind": "app-stopped",
        "appId": app_id,
        "instanceId": instance_id,
    }))
}

#[derive(Debug)]
pub enum StartStopError {
    BadRequest(String),
    Conflict(String),
    Unavailable(String),
}

fn error_response(error: StartStopError) -> Response {
    let (status, message) = match error {
        StartStopError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
        StartStopError::Conflict(message) => (StatusCode::CONFLICT, message),
        StartStopError::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
    };
    (status, Json(json!({ "error": message }))).into_response()
}

/// Autostart targets collected at serve time.
pub async fn autostart_launch_targets(
    http: &HostHttpState,
    targets: &[crate::launch_targets::LaunchTarget],
) {
    for target in targets {
        match start_app_with_launch(
            http,
            target.app_id.as_str(),
            Some(target.document.id.as_str()),
        )
        .await
        {
            Ok(_) => tracing::info!(
                app = %target.app_id,
                launch = %target.document.id,
                "autostarted app from launch config"
            ),
            Err(error) => tracing::warn!(
                app = %target.app_id,
                launch = %target.document.id,
                detail = %match error {
                    StartStopError::BadRequest(m)
                    | StartStopError::Conflict(m)
                    | StartStopError::Unavailable(m) => m,
                },
                "autostart failed"
            ),
        }
    }
}

#[allow(dead_code)]
fn _manifest_touch(manifest: &LaunchManifest) {
    let _ = manifest.revision.clone();
}
