use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::apply_profile::{
    prepare_profile_apply, run_build_request_sync, start_build_request_job, start_profile_apply,
    ApplyProfileRequest,
};
use crate::build_ops::{
    begin_ops_job, build_status_aggregate, finish_ops_job_failure, finish_ops_job_success,
    prebuild_pipeline, reload_pipeline,
};
use crate::state::{HostHttpState, SharedState};
use mei_host_core::BuildRequest;

pub async fn api_host_ops_status(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    Json(build_status_aggregate(&guard))
}

#[derive(Debug, Deserialize, Default)]
pub struct OpsPrebuildBody {
    pub policy: Option<String>,
    pub app_id: Option<String>,
    /// Launch config name (e.g. `data-full`). When set, prebuild uses that
    /// launch's runtime profile + warmup scenes instead of a fixed policy.
    pub config: Option<String>,
}

pub async fn api_host_ops_reload(State(state): State<SharedState>) -> Response {
    let (workspace, app_id) = {
        let mut guard = state.write().expect("state lock");
        let Some(app_id) = guard.default_app().map(str::to_string) else {
            return ops_conflict(
                "Access data plane is unconfigured; apply a workspace profile first".to_string(),
            );
        };
        if let Err(error) = begin_ops_job(&mut guard, "reload") {
            return ops_conflict(error);
        }
        (guard.ctx.workspace_root.clone(), app_id)
    };

    let shell = state.clone();
    let result =
        tokio::task::spawn_blocking(move || reload_pipeline(workspace.as_path(), app_id.as_str()))
            .await
            .map_err(|error| format!("reload task join failed: {error}"))
            .and_then(|inner| inner.map_err(|error| error.to_string()));

    match result {
        Ok(outcome) => {
            let message = if outcome.blocks_changed {
                format!(
                    "reload ok: registry updated + rewarmed (revision={})",
                    outcome.registry_revision
                )
            } else {
                "reload ok: registry unchanged + rewarmed".to_string()
            };
            let payload = json!({
                "accepted": true,
                "kind": "reload",
                "outcome": outcome,
            });
            {
                let mut guard = shell.write().expect("state lock");
                finish_ops_job_success(&mut guard, message);
            }
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(error) => {
            {
                let mut guard = shell.write().expect("state lock");
                finish_ops_job_failure(&mut guard, error.clone());
            }
            ops_failed("reload", error)
        }
    }
}

pub async fn api_host_ops_prebuild(
    State(http): State<HostHttpState>,
    Json(body): Json<OpsPrebuildBody>,
) -> Response {
    let state = http.shell.clone();
    let launch_config = body
        .config
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let policy = body
        .policy
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let (workspace, default_app_id) = {
        let mut guard = state.write().expect("state lock");
        let default_app_id = guard.default_app().map(str::to_string);
        if body
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .is_none()
            && default_app_id.is_none()
        {
            return ops_conflict(
                "Access data plane is unconfigured; choose an app or apply a profile".to_string(),
            );
        }
        if let Err(error) = begin_ops_job(&mut guard, "prebuild") {
            return ops_conflict(error);
        }
        (
            guard.ctx.workspace_root.clone(),
            default_app_id.unwrap_or_default(),
        )
    };
    let app_id = body
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_app_id.as_str())
        .to_string();

    let scenes = if let Some(config_name) = launch_config.as_deref() {
        match mei_host_core::read_launch_config(workspace.as_path(), app_id.as_str(), config_name) {
            Ok(launch) => {
                crate::app_launch_api::apply_launch_runtime_profile(
                    &http,
                    workspace.as_path(),
                    app_id.as_str(),
                    &launch.config,
                );
                crate::app_launch_api::launch_warmup_scenes(
                    workspace.as_path(),
                    &launch.config,
                    app_id.as_str(),
                )
            }
            Err(error) => {
                let mut guard = state.write().expect("state lock");
                finish_ops_job_failure(
                    &mut guard,
                    format!("launch config `{config_name}`: {error}"),
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "accepted": false,
                        "kind": "prebuild",
                        "error": format!("launch config `{config_name}`: {error}"),
                    })),
                )
                    .into_response();
            }
        }
    } else {
        vec![policy.clone()]
    };
    let scenes_for_response = scenes.clone();

    let shell = state.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            prebuild_pipeline(workspace.as_path(), app_id.as_str(), &scenes)
        })
        .await
        .map_err(|error| format!("prebuild task join failed: {error}"))
        .and_then(|inner| inner.map_err(|error| error.to_string()));

        let mut guard = shell.write().expect("state lock");
        match result {
            Ok(build_id) => {
                finish_ops_job_success(
                    &mut guard,
                    format!("prebuild complete (envVersion={build_id})"),
                );
            }
            Err(error) => finish_ops_job_failure(&mut guard, error),
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "accepted": true,
            "kind": "prebuild",
            "config": launch_config,
            "scenes": scenes_for_response,
            "policy": if launch_config.is_some() {
                Value::Null
            } else {
                json!(policy)
            },
        })),
    )
        .into_response()
}

pub async fn api_host_runtime_apply_profile(
    State(http_state): State<HostHttpState>,
    Json(body): Json<ApplyProfileRequest>,
) -> Response {
    let state = http_state.shell.clone();
    {
        let guard = state.read().expect("state lock");
        if guard
            .ops_job
            .as_ref()
            .is_some_and(crate::build_ops::OpsJobState::is_running)
        {
            return ops_conflict("another host-shell ops job is already running".to_string());
        }
    }
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let prepared = match prepare_profile_apply(workspace.as_path(), &body) {
        Ok(prepared) => prepared,
        Err(error) => return apply_profile_error(error),
    };
    if body.dry_run {
        return (
            StatusCode::OK,
            Json(json!({
                "accepted": false,
                "dryRun": true,
                "plan": prepared.plan,
            })),
        )
            .into_response();
    }
    let plan = prepared.plan.clone();
    if let Err(error) = start_profile_apply(
        state,
        http_state.managed_plug,
        http_state.app_runtime,
        prepared,
    ) {
        return ops_conflict(error);
    }
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "accepted": true,
            "kind": "apply-profile",
            "plan": plan,
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostBuildRequestBody {
    #[serde(flatten)]
    pub request: BuildRequest,
    /// When true, wait for the Build Worker and return BuildResult synchronously.
    #[serde(default)]
    pub wait: bool,
}

pub async fn api_host_builds_request(
    State(http_state): State<HostHttpState>,
    Json(body): Json<HostBuildRequestBody>,
) -> Response {
    let state = http_state.shell.clone();
    {
        let guard = state.read().expect("state lock");
        if guard
            .ops_job
            .as_ref()
            .is_some_and(crate::build_ops::OpsJobState::is_running)
        {
            return ops_conflict("another host-shell ops job is already running".to_string());
        }
    }
    if let Err(error) = body.request.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_build_request",
                    "message": error,
                }
            })),
        )
            .into_response();
    }
    if body.wait {
        let result = match tokio::task::spawn_blocking({
            let state = state.clone();
            let request = body.request.clone();
            move || run_build_request_sync(&state, &request)
        })
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": {
                            "code": "build_failed",
                            "message": error,
                        }
                    })),
                )
                    .into_response();
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": {
                            "code": "build_join_failed",
                            "message": error.to_string(),
                        }
                    })),
                )
                    .into_response();
            }
        };
        return (
            StatusCode::OK,
            Json(json!({
                "accepted": true,
                "kind": "build",
                "wait": true,
                "result": result,
            })),
        )
            .into_response();
    }
    match start_build_request_job(state, body.request) {
        Ok(job_id) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "accepted": true,
                "kind": "build",
                "jobId": job_id,
                "wait": false,
            })),
        )
            .into_response(),
        Err(error) => ops_conflict(error),
    }
}

fn apply_profile_error(error: mei_lang_kernel::WorkspaceProfileError) -> Response {
    let (status, code, details) = match error {
        mei_lang_kernel::WorkspaceProfileError::InvalidId => {
            (StatusCode::BAD_REQUEST, "invalid_profile_id", None)
        }
        mei_lang_kernel::WorkspaceProfileError::NotFound => {
            (StatusCode::NOT_FOUND, "profile_not_found", None)
        }
        mei_lang_kernel::WorkspaceProfileError::InvalidPath => {
            (StatusCode::BAD_REQUEST, "invalid_profile_path", None)
        }
        mei_lang_kernel::WorkspaceProfileError::InvalidJson(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_profile_json",
            Some(json!({"parseError": message})),
        ),
        mei_lang_kernel::WorkspaceProfileError::InvalidSchema(issues) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_profile_schema",
            Some(json!({"issues": issues})),
        ),
        mei_lang_kernel::WorkspaceProfileError::RevisionConflict { expected, current } => (
            StatusCode::CONFLICT,
            "revision_conflict",
            Some(json!({"expectedRevision": expected, "currentRevision": current})),
        ),
        mei_lang_kernel::WorkspaceProfileError::Io(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "profile_io_failed", None)
        }
    };
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": "workspace profile apply failed",
                "details": details,
            }
        })),
    )
        .into_response()
}

fn ops_conflict(message: String) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "error": message }))).into_response()
}

fn ops_failed(kind: &str, error: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": error,
            "kind": kind,
        })),
    )
        .into_response()
}
