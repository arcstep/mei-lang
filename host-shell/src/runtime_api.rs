use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::json;

use mei_lang_kernel::discover_apps;

use crate::runtime_snapshot::build_runtime_snapshot;
use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct RuntimeSnapshotQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ScopeActivationQuery {
    pub scope: String,
    pub hops: Option<usize>,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
}

pub async fn api_runtime_snapshot(
    State(state): State<SharedState>,
    Query(params): Query<RuntimeSnapshotQuery>,
) -> Response {
    let app_id = params.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "appId is required"})),
        )
            .into_response();
    }
    let guard = state.read().expect("state lock");
    let discovered = discover_apps(guard.ctx.workspace_root.as_path()).unwrap_or_default();
    if !discovered.iter().any(|app| app.id == app_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("unknown app `{app_id}`")})),
        )
            .into_response();
    }
    let workspace = guard.ctx.workspace_root.as_path();
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace, app_id);
    let snapshot = build_runtime_snapshot(&guard, app_id);
    (StatusCode::OK, Json(snapshot)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct MrgStatusQuery {
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
}

pub async fn api_host_mrg_status(
    State(state): State<SharedState>,
    Query(params): Query<MrgStatusQuery>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let app_id = params
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(guard.ctx.app_id.as_str());
    let workspace = guard.ctx.workspace_root.as_path();
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace, app_id);
    match mei_host_graph::mrg_status_json(workspace, app_id) {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

pub async fn api_host_mrg_activate(
    State(state): State<SharedState>,
    Query(params): Query<ScopeActivationQuery>,
) -> impl IntoResponse {
    let scope = params.scope.trim().to_string();
    if scope.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "scope is required"})),
        )
            .into_response();
    }
    let (workspace, app_id, hops, endpoint) = {
        let guard = state.read().expect("state lock");
        let app_id = params
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(guard.ctx.app_id.as_str())
            .to_string();
        let endpoint = match guard.plug_ds_endpoint_for(app_id.as_str()) {
            Some(endpoint) => endpoint.to_string(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": format!("plug-ds endpoint missing for app `{app_id}`"),
                        "scope": scope,
                    })),
                )
                    .into_response();
            }
        };
        (
            guard.ctx.workspace_root.clone(),
            app_id,
            params.hops.unwrap_or(1).max(1),
            endpoint,
        )
    };

    if let Err(error) =
        crate::plug_proxy::proxy_plug_ds_activate(endpoint.as_str(), scope.as_str(), hops).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error, "scope": scope, "hops": hops, "appId": app_id})),
        )
            .into_response();
    }
    mei_host_graph::record_scope_activation();
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace.as_path(), app_id.as_str());
    let payload = mei_host_graph::build_client_bootstrap_payload(
        workspace.as_path(),
        app_id.as_str(),
        scope.as_str(),
    );
    (
        StatusCode::OK,
        Json(json!({
            "scope": scope,
            "hops": hops,
            "appId": app_id,
            "payload": payload,
        })),
    )
        .into_response()
}
