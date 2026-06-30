use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use mei_host_core::HostContext;
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
pub struct PlugState {
    pub ctx: HostContext,
    /// Serializes scope activation warmups so concurrent MRG activate requests
    /// do not race on shared registry / bootstrap writes.
    activation_lock: Arc<Mutex<()>>,
}

pub fn router(ctx: HostContext) -> Router {
    let state = Arc::new(PlugState {
        ctx,
        activation_lock: Arc::new(Mutex::new(())),
    });
    Router::new()
        .route("/api/plug-ds/health", get(api_health))
        .route(
            "/api/datasets/query/:app_id",
            post(api_datasets_query_with_app),
        )
        .route("/api/datasets/query", post(api_datasets_query))
        .route("/api/datasets/metrics/:app_id", post(api_datasets_metrics))
        .route("/api/plug-ds/activate", post(api_plug_ds_activate))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct ActivationQuery {
    pub scope: String,
    pub hops: Option<usize>,
}

async fn api_health() -> impl IntoResponse {
    Json(json!({ "ok": true, "plug": "mei-plug-ds" }))
}

async fn api_datasets_query(
    State(state): State<Arc<PlugState>>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    match crate::plugin::query_dataset(&state.ctx, &body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => {
            tracing::warn!(
                app_id = %state.ctx.app_id,
                error = %error,
                "dataset query failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response()
        }
    }
}

async fn api_datasets_query_with_app(
    State(state): State<Arc<PlugState>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if state.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    api_datasets_query(State(state), axum::Json(body)).await
}

async fn api_plug_ds_activate(
    State(state): State<Arc<PlugState>>,
    Query(params): Query<ActivationQuery>,
) -> Response {
    let scope = params.scope.trim();
    if scope.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "scope is required"})),
        )
            .into_response();
    }
    let hops = params.hops.unwrap_or(1).max(1);
    let _activation_guard = state
        .activation_lock
        .lock()
        .expect("activation lock poisoned");
    match crate::smart_warmup::run_activation_warmup(&state.ctx, scope, hops) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "accepted": true,
                "scope": scope,
                "hops": hops,
                "appId": state.ctx.app_id,
            })),
        )
            .into_response(),
        Err(error) => {
            tracing::warn!(
                app_id = %state.ctx.app_id,
                scope = %scope,
                hops = hops,
                error = %error,
                "scope activation warmup failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error.to_string(),
                    "scope": scope,
                    "hops": hops,
                })),
            )
                .into_response()
        }
    }
}

async fn api_datasets_metrics(
    State(state): State<Arc<PlugState>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if state.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    match crate::plugin::query_metrics(&state.ctx, &body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => {
            tracing::warn!(
                app_id = %state.ctx.app_id,
                error = %error,
                "metric query failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_router(workspace: std::path::PathBuf, app_id: &str) -> Router {
        router(HostContext::new(workspace, app_id.to_string()))
    }

    #[tokio::test]
    async fn activate_requires_scope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = test_router(tmp.path().to_path_buf(), "data-demo");
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plug-ds/activate?scope=")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn activate_reports_error_on_minimal_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","defaultApp":"data-demo"}}"#,
        )
        .expect("write workspace");
        std::fs::create_dir_all(tmp.path().join("apps/data-demo")).expect("create app dir");
        std::fs::write(
            tmp.path().join("apps/data-demo/app.config.json"),
            r#"{"schemaVersion":1}"#,
        )
        .expect("write app config");
        let app = test_router(tmp.path().to_path_buf(), "data-demo");
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plug-ds/activate?scope=home&hops=1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["scope"], "home");
        assert_eq!(value["hops"], 1);
        assert!(value.get("error").and_then(|error| error.as_str()).is_some());
    }
}
