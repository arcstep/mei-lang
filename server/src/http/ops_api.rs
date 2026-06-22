//! 受限运维面：只读写 ops registry / journal，不写 `.mei`。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    apply_ops_patch_with_journal, journal_path, load_mei_config_for_app,
    resolve_app_root as kernel_resolve_app_root, resolve_mei_config_path, MeiConfig,
    OpsConfigPatch, OpsJournal,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{auth::AuthPrincipal, AppState};

fn resolve_app_root(state: &AppState, app_id: &str) -> Option<std::path::PathBuf> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return None;
    }
    let root = kernel_resolve_app_root(state.source_root.as_path(), app_id);
    if root.is_dir() {
        Some(root)
    } else {
        None
    }
}

#[derive(Debug, Serialize)]
struct OpsConfigResponse {
    app_id: String,
    config: MeiConfig,
    journal_revision: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpsPatchRequest {
    #[serde(default)]
    actor: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    patch: OpsConfigPatch,
}

pub async fn ops_config_get(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let source_root = state.source_root.as_path();
    let config = load_mei_config_for_app(&app_root, Some(source_root));
    let journal = OpsJournal::load(&app_root);
    (
        StatusCode::OK,
        Json(OpsConfigResponse {
            app_id,
            config,
            journal_revision: journal.revision,
        }),
    )
        .into_response()
}

pub async fn ops_config_put(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
    Path(app_id): Path<String>,
    Json(body): Json<OpsPatchRequest>,
) -> impl IntoResponse {
    if body.patch.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "empty ops patch"})),
        )
            .into_response();
    }
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let source_root = state.source_root.as_path();
    let config_path = resolve_mei_config_path(&app_root, Some(source_root));
    let principal_actor = principal
        .as_ref()
        .map(|Extension(value)| format!("{}:{}", value.username, value.role_slug()));
    let actor = if let Some(ref actor) = principal_actor {
        actor.as_str()
    } else if body.actor.trim().is_empty() {
        "manage"
    } else {
        body.actor.trim()
    };
    let summary = if body.summary.trim().is_empty() {
        "ops patch"
    } else {
        body.summary.trim()
    };
    match apply_ops_patch_with_journal(&app_root, &config_path, actor, summary, &body.patch) {
        Ok((config, entry)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "revision": entry.revision,
                "config": config,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

pub async fn ops_journal_get(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let journal = OpsJournal::load(&app_root);
    (
        StatusCode::OK,
        Json(json!({
            "app_id": app_id,
            "journal_path": journal_path(&app_root).display().to_string(),
            "journal": journal,
        })),
    )
        .into_response()
}

pub async fn ops_boundary_get() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "mei_source_readonly": true,
            "writable_objects": mei_lang_kernel::OPS_OBJECT_KINDS,
            "config_file": mei_lang_kernel::MEI_CONFIG_FILENAME,
            "journal_file": mei_lang_kernel::OPS_JOURNAL_REL_PATH,
        })),
    )
}
