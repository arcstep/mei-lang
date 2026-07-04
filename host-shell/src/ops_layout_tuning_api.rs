//! layoutTuning overlay hot-read + session draft (parallel to ops.themes).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    apply_ops_patch_with_journal, layout_tuning_overlay_keys, load_mei_config_for_app,
    ops_layout_tuning_revision_digest, resolve_app_root, resolve_mei_config_path, OpsConfigPatch,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::draft_session::{layout_tuning_draft_storage_key, resolve_draft_session_id};
use crate::layout_tuning_draft::{layout_tuning_draft, merge_layout_tuning_overlay};
use crate::layout_tuning_draft_store::{
    load_layout_tuning_draft_from_disk, persist_layout_tuning_draft,
};
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
    let workspace_root = guard.ctx.workspace_root.clone();
    let app_ctx = guard.host_ctx_for_app(app_id);
    let config = load_mei_config_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root.as_path()),
    );
    let draft = layout_tuning_draft(storage_key.as_str()).or_else(|| {
        load_layout_tuning_draft_from_disk(
            workspace_root.as_path(),
            app_id,
            storage_key.as_str(),
        )
    });
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
    State(state): State<SharedState>,
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
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    persist_layout_tuning_draft(
        workspace_root.as_path(),
        app_id,
        storage_key.as_str(),
        &body.tuning,
    );
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

pub async fn api_ops_layout_tuning_apply_post(
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
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let draft = layout_tuning_draft(storage_key.as_str()).or_else(|| {
        load_layout_tuning_draft_from_disk(
            workspace_root.as_path(),
            app_id,
            storage_key.as_str(),
        )
    });
    let Some(draft_value) = draft.filter(|value| !value.is_null()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no active layoutTuning draft"})),
        )
            .into_response();
    };
    let app_root = resolve_app_root(workspace_root.as_path(), app_id);
    let config_path =
        resolve_mei_config_path(app_root.as_path(), Some(workspace_root.as_path()));
    let config = load_mei_config_for_app(
        app_root.as_path(),
        Some(workspace_root.as_path()),
    );
    let merged = merge_layout_tuning_overlay(config.ops.layout_tuning.as_ref(), Some(&draft_value))
        .unwrap_or(draft_value);
    let patch = OpsConfigPatch {
        layout_tuning: Some(merged),
        ..Default::default()
    };
    match apply_ops_patch_with_journal(
        app_root.as_path(),
        config_path.as_path(),
        "build-layout-tuning",
        "apply layoutTuning draft to ops.layoutTuning",
        &patch,
    ) {
        Ok((updated, entry)) => {
            persist_layout_tuning_draft(
                workspace_root.as_path(),
                app_id,
                storage_key.as_str(),
                &Value::Null,
            );
            crate::build_fragment_cache::clear_build_fragment_cache_for_app(app_id);
            crate::access_page_cache::clear_access_page_render_cache_for_app(
                workspace_root.as_path(),
                app_id,
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "revision": entry.revision,
                    "layout_tuning_revision": ops_layout_tuning_revision_digest(&updated.ops),
                })),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}
