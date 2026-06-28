use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use mei_lang_app::scene_theme_style_for_theme_id;
use mei_lang_kernel::{decode_theme_ref_token, load_mei_config_for_app};
use serde_json::json;

use crate::api_stubs::{
    api_agent_config_stub, api_agent_runtime_stub, api_agent_sessions_stub,
    api_agent_skill_stub, api_auth_session_stub,
};
use crate::assets::{app_asset, app_bundle, component_asset, workspace_app_asset};
use crate::build_info::{self, BUILD_VERSION};
use crate::pages::{app_page, index};
use crate::state::SharedState;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/app-assets/favicon.svg") }),
        )
        .route("/", get(index))
        .route("/api/host/heartbeat", get(api_host_heartbeat))
        .route("/api/host/version", get(api_host_version))
        .route("/api/host/ready", get(api_host_ready))
        .route("/api/host/readiness", get(api_host_ready))
        .route("/api/auth/session", get(api_auth_session_stub))
        .route("/api/agent/config", get(api_agent_config_stub))
        .route("/api/agent/runtime", get(api_agent_runtime_stub))
        .route("/api/agent/skill", get(api_agent_skill_stub))
        .route("/api/agent/session", get(api_agent_sessions_stub))
        .route(
            "/api/datasets/query/:app_id",
            post(api_datasets_query_with_app),
        )
        .route("/api/datasets/query", post(api_datasets_query))
        .route(
            "/api/datasets/metrics/:app_id",
            post(api_datasets_metrics),
        )
        .route(
            "/api/ops/theme/style/:app_id",
            get(api_ops_theme_style),
        )
        .route("/apps/:mode/*app_id", get(app_page))
        .route("/app-bundles/:mode", get(app_bundle))
        .route("/app-assets/*path", get(app_asset))
        .route(
            "/workspace-app-assets/:app_id/*path",
            get(workspace_app_asset),
        )
        .route("/workspace-components/*path", get(component_asset))
        .with_state(state)
}

async fn api_host_heartbeat(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let access_ready = guard.imported && guard.warmed_up;
    let app_id = guard.ctx.app_id.clone();
    let workspace_root = guard.ctx.workspace_root.as_path();
    let descriptor = build_info::version_descriptor(
        Some(workspace_root),
        Some(guard.host_started_at_ms),
    );
    let display_label = descriptor
        .get("displayLabel")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    Json(json!({
        "buildVersion": BUILD_VERSION,
        "buildDescriptor": build_info::binary_descriptor(),
        "displayLabel": display_label,
        "version": descriptor,
        "hostStartedAtMs": guard.host_started_at_ms,
        "ready": access_ready,
        "hostReady": access_ready,
        "accessReady": access_ready,
        "anyAppAccessReady": access_ready,
        "defaultAppId": app_id,
        "defaultAppAccessReady": access_ready,
        "apps": [{
            "appId": app_id,
            "accessReady": access_ready,
            "phase": if access_ready { "ready" } else { "starting" },
        }],
    }))
}

async fn api_host_version(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    Json(build_info::version_descriptor(
        Some(guard.ctx.workspace_root.as_path()),
        Some(guard.host_started_at_ms),
    ))
}

async fn api_host_ready(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let ready = guard.imported && guard.warmed_up;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "hostReady": ready,
            "imported": guard.imported,
            "warmedUp": guard.warmed_up,
        })),
    )
}

async fn api_datasets_query(
    State(state): State<SharedState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    api_datasets_query_inner(state, body).await
}

async fn api_datasets_query_with_app(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let _ = app_id;
    api_datasets_query_inner(state, body).await
}

async fn api_datasets_query_inner(
    state: SharedState,
    body: serde_json::Value,
) -> Response {
    let guard = state.read().expect("state lock");
    #[cfg(feature = "ds")]
    {
        match mei_plug_ds::query_dataset(&guard.ctx, &body) {
            Ok(value) => (StatusCode::OK, Json(value)).into_response(),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response(),
        }
    }
    #[cfg(not(feature = "ds"))]
    {
        let _ = (guard, body);
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({"error": "ds feature disabled"})),
        )
            .into_response()
    }
}

async fn api_datasets_metrics(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let guard = state.read().expect("state lock");
    if guard.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    #[cfg(feature = "ds")]
    {
        match mei_plug_ds::query_metrics(&guard.ctx, &body) {
            Ok(value) => (StatusCode::OK, Json(value)).into_response(),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response(),
        }
    }
    #[cfg(not(feature = "ds"))]
    {
        let _ = (guard, body);
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({"error": "ds feature disabled"})),
        )
            .into_response()
    }
}

#[derive(serde::Deserialize)]
struct ThemeStyleQuery {
    theme: Option<String>,
}

async fn api_ops_theme_style(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    Query(query): Query<ThemeStyleQuery>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    if guard.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    let app_root = guard.ctx.app_root();
    let live_config = load_mei_config_for_app(app_root.as_path(), Some(guard.ctx.workspace_root.as_path()));
    let theme_id = query
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .unwrap_or_else(|| "cockpit".to_string());
    let css = scene_theme_style_for_theme_id(theme_id.as_str(), Some(&live_config));
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/css; charset=utf-8",
        )],
        css,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use mei_host_core::HostContext;
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_state(workspace: std::path::PathBuf) -> SharedState {
        Arc::new(RwLock::new(crate::state::ShellState {
            ctx: HostContext::new(workspace, "data-demo".to_string()),
            package_root: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            imported: true,
            warmed_up: true,
            host_started_at_ms: 1,
        }))
    }

    #[tokio::test]
    async fn api_host_version_returns_binary_and_workspace_descriptor() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/version")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            value["binary"]["build_version"].as_str(),
            Some(crate::build_info::BUILD_VERSION)
        );
        assert_eq!(value["hostStartedAtMs"], 1);
        assert!(value.get("workspace").is_some());
    }
}
