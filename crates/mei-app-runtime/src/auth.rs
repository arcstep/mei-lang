use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{HEADER_APP_ID, HEADER_GENERATION, HEADER_INSTANCE_TOKEN};
use serde_json::json;

use crate::state::SharedRuntimeState;

/// Paths that may be probed without an instance token (supervisor health/ready).
pub fn is_public_health_path(path: &str) -> bool {
    matches!(
        path,
        "/api/app-runtime/health" | "/api/app-runtime/ready" | "/api/plug-ds/health"
    )
}

/// Reject Access/data-plane traffic until bootstrap/warmup flips `ready`.
/// Keeps `/health` + `/ready` available so Host can wait without hanging proxies.
pub async fn require_ready_for_data_plane(
    State(state): State<SharedRuntimeState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if is_public_health_path(path.as_str()) || path == "/api/app-runtime/meta" {
        return next.run(request).await;
    }
    let snap = state.snapshot();
    if snap.ready {
        return next.run(request).await;
    }
    let phase = match snap.phase {
        mei_host_core::InstancePhase::Queued => "queued",
        mei_host_core::InstancePhase::Building => "building",
        mei_host_core::InstancePhase::Launching => "launching",
        mei_host_core::InstancePhase::Importing => "importing",
        mei_host_core::InstancePhase::Snapshotting => "snapshotting",
        mei_host_core::InstancePhase::Warming => "warming",
        mei_host_core::InstancePhase::Ready => "ready",
        mei_host_core::InstancePhase::Failed => "failed",
        mei_host_core::InstancePhase::Stopped => "stopped",
    };
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "app-runtime not ready",
            "kind": "app-runtime-not-ready",
            "phase": phase,
            "lastError": snap.last_error,
            "appId": state.app_id(),
        })),
    )
        .into_response()
}

pub async fn require_instance_token(
    State(state): State<SharedRuntimeState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if is_public_health_path(path.as_str()) {
        return next.run(request).await;
    }

    let token = request
        .headers()
        .get(HEADER_INSTANCE_TOKEN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() || token != state.token.as_str() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "missing or invalid x-mei-instance-token",
            })),
        )
            .into_response();
    }

    if let Some(app_header) = request
        .headers()
        .get(HEADER_APP_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if app_header != state.app_id() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "x-mei-app-id does not match this runtime instance",
                    "expected": state.app_id(),
                    "got": app_header,
                })),
            )
                .into_response();
        }
    }

    if let Some(gen_header) = request
        .headers()
        .get(HEADER_GENERATION)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if gen_header != state.generation() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "x-mei-generation does not match this runtime instance",
                    "expected": state.generation(),
                    "got": gen_header,
                })),
            )
                .into_response();
        }
    }

    if let Some(app_from_path) = extract_app_id_from_path(path.as_str()) {
        if app_from_path != state.app_id() {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "app mismatch",
                    "expected": state.app_id(),
                    "got": app_from_path,
                })),
            )
                .into_response();
        }
    }

    next.run(request).await
}

fn extract_app_id_from_path(path: &str) -> Option<&str> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        ["apps", app, ..] => Some(*app),
        ["api", "datasets", "query", app] => Some(*app),
        ["api", "datasets", "metrics", app] => Some(*app),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_paths_are_public() {
        assert!(is_public_health_path("/api/app-runtime/health"));
        assert!(is_public_health_path("/api/app-runtime/ready"));
        assert!(is_public_health_path("/api/plug-ds/health"));
        assert!(!is_public_health_path("/api/app-runtime/meta"));
        assert!(!is_public_health_path("/api/host/view-revision"));
    }

    #[test]
    fn extracts_app_from_access_and_dataset_paths() {
        assert_eq!(
            extract_app_id_from_path("/apps/mini-data/home"),
            Some("mini-data")
        );
        assert_eq!(
            extract_app_id_from_path("/api/datasets/query/mini-data"),
            Some("mini-data")
        );
        assert_eq!(extract_app_id_from_path("/api/app-runtime/health"), None);
    }
}
