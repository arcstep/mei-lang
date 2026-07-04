//! layoutTuning overlay hot-read + session draft (parallel to ops.themes).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    layout_tuning_overlay_keys, load_mei_config_for_app, ops_layout_tuning_revision_digest,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::layout_tuning_draft::{layout_tuning_draft, merge_layout_tuning_overlay, set_layout_tuning_draft};
use crate::state::SharedState;

#[derive(Debug, serde::Serialize)]
struct LayoutTuningOverlayResponse {
    app_id: String,
    revision: String,
    draft_active: bool,
    entries: std::collections::BTreeMap<String, Value>,
}

pub async fn api_ops_layout_tuning_overlay_get(
    State(state): State<SharedState>,
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
    let guard = state.read().expect("state lock");
    let app_ctx = guard.host_ctx_for_app(app_id);
    let config = load_mei_config_for_app(
        app_ctx.app_root().as_path(),
        Some(guard.ctx.workspace_root.as_path()),
    );
    let draft = layout_tuning_draft(app_id);
    let merged = merge_layout_tuning_overlay(config.ops.layout_tuning.as_ref(), draft.as_ref());
    let revision = if draft.is_some() {
        format!(
            "{}+draft",
            ops_layout_tuning_revision_digest(&config.ops)
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
    set_layout_tuning_draft(app_id, body.tuning.clone());
    let draft_active = !body.tuning.is_null();
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "app_id": app_id,
            "draft": draft_active,
        })),
    )
        .into_response()
}
