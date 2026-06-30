use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::json;

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
    if guard.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    let snapshot = build_runtime_snapshot(&guard);
    (StatusCode::OK, Json(snapshot)).into_response()
}

pub async fn api_host_mrg_status(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    match mei_host_graph::mrg_status_json(
        guard.ctx.workspace_root.as_path(),
        guard.ctx.app_id.as_str(),
    ) {
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
    let (endpoint, workspace, app_id, hops) = {
        let guard = state.read().expect("state lock");
        (
            guard.plug_ds_endpoint.clone(),
            guard.ctx.workspace_root.clone(),
            guard.ctx.app_id.clone(),
            params.hops.unwrap_or(1).max(1),
        )
    };

    if let Err(error) =
        crate::plug_proxy::proxy_plug_ds_activate(endpoint.as_str(), scope.as_str(), hops).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error, "scope": scope, "hops": hops})),
        )
            .into_response();
    }
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
            "payload": payload,
        })),
    )
        .into_response()
}
