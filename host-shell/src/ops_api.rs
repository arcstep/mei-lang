use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::build_ops::{
    begin_ops_job, build_status_aggregate, finish_ops_job_failure, finish_ops_job_success,
    prebuild_pipeline, reload_pipeline,
};
use crate::state::SharedState;

pub async fn api_host_ops_status(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    Json(build_status_aggregate(&guard))
}

#[derive(Debug, Deserialize, Default)]
pub struct OpsPrebuildBody {
    pub policy: Option<String>,
    pub app_id: Option<String>,
}

pub async fn api_host_ops_reload(State(state): State<SharedState>) -> Response {
    let (workspace, app_id) = {
        let mut guard = state.write().expect("state lock");
        if let Err(error) = begin_ops_job(&mut guard, "reload") {
            return ops_conflict(error);
        }
        (guard.ctx.workspace_root.clone(), guard.ctx.app_id.clone())
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
    State(state): State<SharedState>,
    Json(body): Json<OpsPrebuildBody>,
) -> Response {
    let policy = body
        .policy
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let (workspace, default_app_id) = {
        let mut guard = state.write().expect("state lock");
        if let Err(error) = begin_ops_job(&mut guard, "prebuild") {
            return ops_conflict(error);
        }
        (guard.ctx.workspace_root.clone(), guard.ctx.app_id.clone())
    };
    let app_id = body
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_app_id.as_str())
        .to_string();

    let shell = state.clone();
    let policy_for_response = policy.clone();
    tokio::spawn(async move {
        let policy_for_task = policy;
        let result = tokio::task::spawn_blocking(move || {
            prebuild_pipeline(
                workspace.as_path(),
                app_id.as_str(),
                policy_for_task.as_str(),
            )
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
            "policy": policy_for_response,
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
