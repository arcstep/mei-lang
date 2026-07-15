use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use mei_host_core::InstancePhase;
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::access::{access_app_root, access_app_stage};
use crate::auth::require_instance_token;
use crate::host_data::{
    api_host_layer_batch, api_host_scene_manifest, api_host_view_revision, api_scene_bootstrap,
    api_scene_eval_pack,
};
use crate::state::SharedRuntimeState;

pub fn router(state: SharedRuntimeState) -> Router {
    let ds = mei_plug_ds::http_router(state.host.clone());

    let runtime_api = Router::new()
        .route("/api/app-runtime/health", get(api_health))
        .route("/api/app-runtime/ready", get(api_ready))
        .route("/api/app-runtime/meta", get(api_meta))
        .route("/api/host/view-revision", get(api_host_view_revision))
        .route("/api/host/scene-manifest", get(api_host_scene_manifest))
        .route("/api/host/layer-batch", post(api_host_layer_batch))
        .route("/api/host/scene-eval-pack", get(api_scene_eval_pack))
        .route("/api/host/scene-bootstrap", get(api_scene_bootstrap))
        .route("/api/datasets/fixture/:app_id", post(api_datasets_fixture))
        .route("/apps/:app_id", get(access_app_root))
        .route(
            "/apps/:app_id/~/*target_tail",
            get(crate::access::access_app_temp_stage),
        )
        .route(
            "/apps/:app_id/:stage/*scoped_tail",
            get(crate::access::access_app_scoped_stage),
        )
        .route("/apps/:app_id/:stage", get(access_app_stage))
        .with_state(state.clone());

    Router::new()
        .merge(runtime_api)
        .merge(ds)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_ready_for_data_plane,
        ))
        .layer(middleware::from_fn_with_state(
            state,
            require_instance_token,
        ))
        .layer(TraceLayer::new_for_http())
}

async fn api_datasets_fixture(
    State(state): State<SharedRuntimeState>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if let Some(ceiling) = state
        .spec
        .data_mode_ceiling
        .as_deref()
        .and_then(mei_lang_kernel::DataModeCeiling::parse)
    {
        if matches!(ceiling, mei_lang_kernel::DataModeCeiling::Static) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "fixture datasets API unavailable under static data mode ceiling"
                })),
            )
                .into_response();
        }
    }
    let scene_id = body
        .get("scene_id")
        .or_else(|| body.get("sceneId"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    let workspace = state.host.workspace_root.as_path();
    let Some(manifest) =
        mei_host_graph::read_client_bootstrap(workspace, app_id.as_str(), scene_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("fixture bootstrap missing for scene `{scene_id}`")})),
        )
            .into_response();
    };
    (
        StatusCode::OK,
        Json(json!({
            "source": "fixture",
            "app_id": app_id,
            "scene_id": scene_id,
            "client_revision": manifest.client_revision,
            "metrics": manifest.metrics,
        })),
    )
        .into_response()
}

async fn api_health() -> impl IntoResponse {
    Json(json!({ "ok": true, "plug": "app-runtime" }))
}

async fn api_ready(State(state): State<SharedRuntimeState>) -> impl IntoResponse {
    let snap = state.snapshot();
    let phase = phase_slug(snap.phase);
    Json(json!({
        "ok": snap.ready,
        "ready": snap.ready,
        "phase": phase,
        "lastError": snap.last_error,
        "revisions": {
            "registryRevision": snap.revisions.registry_revision,
            "clientRevision": snap.revisions.client_revision,
            "dataGeneration": snap.revisions.data_generation,
        },
        "appId": state.app_id(),
        "generation": state.generation(),
        "instanceId": state.instance_id(),
    }))
}

async fn api_meta(State(state): State<SharedRuntimeState>) -> impl IntoResponse {
    let snap = state.snapshot();
    Json(json!({
        "ok": true,
        "appId": state.app_id(),
        "generation": state.generation(),
        "instanceId": state.instance_id(),
        "specDigest": state.spec_digest(),
        "phase": phase_slug(snap.phase),
        "ready": snap.ready,
        "revisions": {
            "registryRevision": snap.revisions.registry_revision,
            "clientRevision": snap.revisions.client_revision,
            "dataGeneration": snap.revisions.data_generation,
        },
        "runtimeAbi": state.spec.runtime_abi,
        "profileId": state.spec.config_snapshot.profile_id,
    }))
}

fn phase_slug(phase: InstancePhase) -> &'static str {
    match phase {
        InstancePhase::Queued => "queued",
        InstancePhase::Building => "building",
        InstancePhase::Launching => "launching",
        InstancePhase::Importing => "importing",
        InstancePhase::Snapshotting => "snapshotting",
        InstancePhase::Warming => "warming",
        InstancePhase::Ready => "ready",
        InstancePhase::Failed => "failed",
        InstancePhase::Stopped => "stopped",
    }
}

/// Route paths registered by [`router`] (for tests / introspection).
pub fn registered_route_paths() -> &'static [&'static str] {
    &[
        "/api/app-runtime/health",
        "/api/app-runtime/ready",
        "/api/app-runtime/meta",
        "/api/plug-ds/health",
        "/api/datasets/query",
        "/api/datasets/query/:app_id",
        "/api/datasets/metrics/:app_id",
        "/api/plug-ds/activate",
        "/api/host/view-revision",
        "/api/host/scene-manifest",
        "/api/host/layer-batch",
        "/api/host/scene-eval-pack",
        "/api/host/scene-bootstrap",
        "/apps/:app_id",
        "/apps/:app_id/:stage",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_routes_cover_required_surface() {
        let paths = registered_route_paths();
        assert!(paths.contains(&"/api/app-runtime/health"));
        assert!(paths.contains(&"/api/app-runtime/ready"));
        assert!(paths.contains(&"/api/app-runtime/meta"));
        assert!(paths.contains(&"/api/plug-ds/health"));
        assert!(paths.contains(&"/api/datasets/query"));
        assert!(paths.contains(&"/api/host/view-revision"));
        assert!(paths.contains(&"/api/host/scene-manifest"));
        assert!(paths.contains(&"/api/host/layer-batch"));
        assert!(paths.contains(&"/api/host/scene-eval-pack"));
        assert!(paths.contains(&"/api/host/scene-bootstrap"));
        assert!(paths.contains(&"/apps/:app_id"));
        assert!(paths.contains(&"/apps/:app_id/:stage"));
    }
}
