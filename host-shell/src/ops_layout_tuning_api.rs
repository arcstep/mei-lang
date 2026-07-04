//! layoutTuning overlay hot-read + session draft (parallel to ops.themes).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    layout_tuning_overlay_keys, load_mei_config_for_app, ops_layout_tuning_revision_digest,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::draft_session::{layout_tuning_draft_storage_key, resolve_draft_session_id};
use crate::layout_tuning_draft::{layout_tuning_draft, merge_layout_tuning_overlay, set_layout_tuning_draft};
use crate::state::SharedState;

#[derive(Debug, serde::Serialize)]
struct LayoutTuningOverlayResponse {
    app_id: String,
    session_id: String,
    revision: String,
    draft_active: bool,
    entries: std::collections::BTreeMap<String, Value>,
}

pub async fn api_ops_layout_tuning_overlay_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let session_id = resolve_draft_session_id(&headers);
    let storage_key = layout_tuning_draft_storage_key(app_id, session_id.as_str());
    let guard = state.read().expect("state lock");
    let app_ctx = guard.host_ctx_for_app(app_id);
    let config = load_mei_config_for_app(
        app_ctx.app_root().as_path(),
        Some(guard.ctx.workspace_root.as_path()),
    );
    let draft = layout_tuning_draft(storage_key.as_str());
    let merged = merge_layout_tuning_overlay(config.ops.layout_tuning.as_ref(), draft.as_ref());
    let revision = if draft.is_some() {
        format!(
            "{}+draft:{}",
            ops_layout_tuning_revision_digest(&config.ops),
            session_id
        )
    } else {
        ops_layout_tuning_revision_digest(&config.ops)
    };
    let entries = merged
        .as_ref()
        .map(layout_tuning_overlay_keys)
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(LayoutTuningOverlayResponse {
            app_id: app_id.to_string(),
            session_id,
            revision,
            draft_active: draft.is_some(),
            entries,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct LayoutTuningDraftRequest {
    #[serde(default)]
    pub tuning: Value,
}

pub async fn api_ops_layout_tuning_draft_put(
    State(_state): State<SharedState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    Json(body): Json<LayoutTuningDraftRequest>,
) -> impl IntoResponse {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let session_id = resolve_draft_session_id(&headers);
    let storage_key = layout_tuning_draft_storage_key(app_id, session_id.as_str());
    set_layout_tuning_draft(storage_key.as_str(), body.tuning.clone());
    let draft_active = !body.tuning.is_null();
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "app_id": app_id,
            "session_id": session_id,
            "draft": draft_active,
        })),
    )
        .into_response()
}
