//! HTTP APIs for per-app launch configs and start/stop (0537).

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{
    ensure_default_launch_config, list_launch_configs, read_launch_config, write_launch_config,
    AppLaunchConfig, AppLaunchDocument, DesiredInstance, DesiredState, LaunchManifest,
    RouteBinding,
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
    let config_ref = config.unwrap_or("default");
    let launch = match read_launch_config(workspace.as_path(), app_id, config_ref) {
        Ok(doc) => doc,
        Err(_) if config_ref == "default" => {
            ensure_default_launch_config(workspace.as_path(), app_id)
                .map_err(|e| StartStopError::BadRequest(e.to_string()))?
        }
        Err(error) => return Err(StartStopError::BadRequest(error.to_string())),
    };
    apply_launch_runtime_profile(http, workspace.as_path(), &launch.config);
    prepare_current_launch(http, workspace.as_path(), app_id, &launch).await?;

    // Keep the active instance serving until prebuild succeeds, then enforce
    // the single-process rule immediately before spawning its replacement.
    let _ = stop_app_runtime(http, app_id).await;
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
    sync_host_control_runtime_plan(
        workspace.as_path(),
        &spec_for_state.config_snapshot.runtime_plan,
    );
    crate::dev_eval_scope::install_runtime_plan(
        spec_for_state.config_snapshot.runtime_plan.clone(),
    );

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
            crate::shell_chrome::running_event_payload_with_plan(
                workspace.as_path(),
                app_id,
                launch.id.as_str(),
                spec_for_state.instance_id.as_str(),
                Some(&spec_for_state.config_snapshot.runtime_plan),
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

async fn prepare_current_launch(
    http: &HostHttpState,
    workspace: &std::path::Path,
    app_id: &str,
    launch: &AppLaunchDocument,
) -> Result<(), StartStopError> {
    if !launch_uses_current_generation(&launch.config) {
        return Ok(());
    }
    {
        let guard = http.shell.read().expect("state lock");
        let _ = guard.events.send(HostEvent::new(
            "app-starting",
            json!({
                "appId": app_id,
                "launchId": launch.id,
                "phase": "prebuilding",
                "message": "正在编译应用并准备数据快照…",
            }),
        ));
    }
    let workspace = workspace.to_path_buf();
    let app_id = app_id.to_string();
    let scenes = launch_warmup_scenes(&launch.config, app_id.as_str());
    tokio::task::spawn_blocking(move || {
        crate::build_ops::prebuild_pipeline(workspace.as_path(), app_id.as_str(), &scenes)
    })
    .await
    .map_err(|error| StartStopError::Unavailable(format!("app prebuild task failed: {error}")))?
    .map_err(|error| StartStopError::Unavailable(format!("app prebuild failed: {error}")))?;
    Ok(())
}

fn launch_uses_current_generation(config: &AppLaunchConfig) -> bool {
    let generation = config.generation.trim();
    generation.is_empty() || generation.eq_ignore_ascii_case("current")
}

fn launch_runtime_plan(
    workspace: &std::path::Path,
    config: &AppLaunchConfig,
) -> mei_lang_kernel::RuntimePlan {
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    if let Some(value) = config.runtime_plan.as_ref() {
        return serde_json::from_value(value.clone()).unwrap_or(RuntimePlan {
            default_mode: RuntimeMode::Lazy,
            apps: Default::default(),
        });
    }
    let path = workspace.join("deploy/applied/runtime-plan.json");
    if path.is_file() {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(plan) = serde_json::from_str::<RuntimePlan>(&raw) {
                return plan;
            }
        }
    }
    RuntimePlan {
        default_mode: RuntimeMode::Lazy,
        apps: Default::default(),
    }
}

fn apply_launch_data_mode_ceiling(http: &HostHttpState, config: &AppLaunchConfig) {
    let Some(raw) = config
        .data_mode_ceiling
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(ceiling) = mei_lang_kernel::DataModeCeiling::parse(raw) else {
        tracing::warn!(
            ceiling = %raw,
            "launch config dataModeCeiling ignored: unknown value"
        );
        return;
    };
    let mut guard = http.shell.write().expect("state lock");
    guard.data_mode_ceiling = ceiling;
}

fn apply_launch_runtime_profile(
    http: &HostHttpState,
    workspace: &std::path::Path,
    config: &AppLaunchConfig,
) {
    let runtime_plan = launch_runtime_plan(workspace, config);
    sync_host_control_runtime_plan(workspace, &runtime_plan);
    crate::dev_eval_scope::install_runtime_plan(runtime_plan);
    apply_launch_data_mode_ceiling(http, config);
}

fn launch_warmup_enabled(config: &AppLaunchConfig) -> bool {
    config
        .warmup
        .as_ref()
        .and_then(|warmup| warmup.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

/// All `hotScenes` from launch warmup config (not just the first).
fn launch_warmup_scenes(config: &AppLaunchConfig, app_id: &str) -> Vec<String> {
    if !launch_warmup_enabled(config) {
        return Vec::new();
    }
    let scenes = config
        .warmup
        .as_ref()
        .and_then(|warmup| warmup.get("apps"))
        .and_then(|apps| apps.get(app_id))
        .and_then(|app| app.get("hotScenes"))
        .and_then(serde_json::Value::as_array)
        .map(|scenes| {
            scenes
                .iter()
                .filter_map(|scene| scene.as_str())
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if scenes.is_empty() {
        vec!["home".to_string()]
    } else {
        scenes
    }
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

/// Mirror launch-bound runtimePlan into host-control so legacy readers stay consistent.
fn sync_host_control_runtime_plan(
    workspace: &std::path::Path,
    runtime_plan: &mei_lang_kernel::RuntimePlan,
) {
    let mut control = mei_host_core::read_host_control_state(workspace)
        .unwrap_or_else(mei_host_core::HostControlState::empty);
    control.runtime_plan = Some(runtime_plan.clone());
    if let Err(error) = mei_host_core::write_host_control_state(workspace, &control) {
        tracing::warn!(
            error = %error,
            "failed to sync runtimePlan into host-control after app start"
        );
    }
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

#[cfg(test)]
mod tests {
    use super::{launch_uses_current_generation, launch_warmup_scenes};
    use mei_host_core::AppLaunchConfig;

    #[test]
    fn current_generation_launch_requires_prebuild() {
        let mut config = AppLaunchConfig::default_for_app("pretty-panels");
        config.generation = "current".to_string();
        assert!(launch_uses_current_generation(&config));

        config.generation = "WS-20260713.0".to_string();
        assert!(!launch_uses_current_generation(&config));
    }

    #[test]
    fn warmup_scenes_include_all_hot_scenes() {
        let mut config = AppLaunchConfig::default_for_app("pretty-panels");
        config.warmup = Some(serde_json::json!({
            "enabled": true,
            "apps": {
                "pretty-panels": {
                    "hotScenes": ["home/t1", "home"]
                }
            }
        }));
        assert_eq!(
            launch_warmup_scenes(&config, "pretty-panels"),
            vec!["home/t1".to_string(), "home".to_string()]
        );
        assert_eq!(
            launch_warmup_scenes(&config, "other-app"),
            vec!["home".to_string()]
        );
    }

    #[test]
    fn warmup_disabled_skips_all_scenes() {
        let mut config = AppLaunchConfig::default_for_app("pretty-panels");
        config.warmup = Some(serde_json::json!({
            "enabled": false,
            "apps": {
                "pretty-panels": {
                    "hotScenes": ["home"]
                }
            }
        }));
        assert!(launch_warmup_scenes(&config, "pretty-panels").is_empty());
    }
}

#[allow(dead_code)]
fn _manifest_touch(manifest: &LaunchManifest) {
    let _ = manifest.revision.clone();
}
