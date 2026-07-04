use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::{
    apply_ops_patch_with_journal, load_mei_config_for_app, resolve_app_root,
    resolve_mei_config_path, MeiConfig, OpsConfigPatch, OpsJournal, OPS_JOURNAL_REL_PATH,
    OPS_OBJECT_KINDS, MEI_CONFIG_FILENAME,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::SharedState;

#[derive(Debug, Serialize)]
struct OpsConfigResponse {
    app_id: String,
    config: MeiConfig,
    journal_revision: u64,
}

#[derive(Debug, Deserialize)]
pub struct OpsPatchRequest {
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub patch: OpsConfigPatch,
}

pub async fn ops_config_get(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let app_root = resolve_app_root(workspace_root.as_path(), app_id.as_str());
    if !app_root.is_dir() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root.as_path()));
    let journal = OpsJournal::load(app_root.as_path());
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
    State(state): State<SharedState>,
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
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let app_root = resolve_app_root(workspace_root.as_path(), app_id.as_str());
    if !app_root.is_dir() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let config_path = resolve_mei_config_path(app_root.as_path(), Some(workspace_root.as_path()));
    let actor = principal
        .as_ref()
        .map(|Extension(value)| format!("{}:{}", value.username, value.role_slug()))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if body.actor.trim().is_empty() {
                "manage".to_string()
            } else {
                body.actor.trim().to_string()
            }
        });
    let summary = if body.summary.trim().is_empty() {
        "ops patch"
    } else {
        body.summary.trim()
    };
    match apply_ops_patch_with_journal(
        app_root.as_path(),
        config_path.as_path(),
        actor.as_str(),
        summary,
        &body.patch,
    ) {
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

pub async fn ops_boundary_get() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "mei_source_readonly": true,
            "writable_objects": OPS_OBJECT_KINDS,
            "config_file": MEI_CONFIG_FILENAME,
            "journal_file": OPS_JOURNAL_REL_PATH,
        })),
    )
        .into_response()
}

pub async fn ops_journal_get(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let app_root = resolve_app_root(workspace_root.as_path(), app_id.as_str());
    if !app_root.is_dir() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let journal = OpsJournal::load(app_root.as_path());
    (
        StatusCode::OK,
        Json(json!({
            "app_id": app_id,
            "journal_path": app_root.join(OPS_JOURNAL_REL_PATH).display().to_string(),
            "journal": journal,
        })),
    )
        .into_response()
}
