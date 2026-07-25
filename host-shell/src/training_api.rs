//! Item-based mastery training HTTP API (built-in SM-2 progress store).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::resolve_app_root;
use mei_training::{
    load_wubi_catalog, next_item, now_millis, review_item, session_summary, LadderStage,
    LearnerStore, NextRequest, PracticeIntent, ReviewRequest, TrainingMode, WubiCatalog,
};
use serde::Deserialize;
use serde_json::json;

use crate::state::SharedState;

fn catalog_cache() -> &'static Mutex<HashMap<String, WubiCatalog>> {
    static CACHE: OnceLock<Mutex<HashMap<String, WubiCatalog>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

const DEFAULT_DEV_LEARNER: &str = "dev";

/// Prefer authenticated username; when auth is off / anonymous, use `dev`.
fn resolve_learner_id(principal: Option<Extension<AuthPrincipal>>) -> String {
    principal
        .as_ref()
        .map(|Extension(p)| p.username.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_DEV_LEARNER.to_string())
}

fn load_catalog(workspace: &std::path::Path, app_id: &str) -> Result<WubiCatalog, Response> {
    if app_id != "wubi" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "unsupported_app",
                "message": format!("training catalog not configured for app `{app_id}`")
            })),
        )
            .into_response());
    }
    {
        let cache = catalog_cache().lock().expect("catalog cache");
        if let Some(hit) = cache.get(app_id) {
            return Ok(hit.clone());
        }
    }
    let app_root = resolve_app_root(workspace, app_id);
    let catalog = load_wubi_catalog(app_root.as_path()).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "catalog_load_failed", "message": err.to_string() })),
        )
            .into_response()
    })?;
    catalog_cache()
        .lock()
        .expect("catalog cache")
        .insert(app_id.to_string(), catalog.clone());
    Ok(catalog)
}

pub async fn api_training_session(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Path(app_id): Path<String>,
) -> Response {
    let username = resolve_learner_id(principal);
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let catalog = match load_catalog(workspace.as_path(), app_id.as_str()) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    let store = LearnerStore::open(workspace.as_path(), app_id.as_str(), username.as_str());
    match session_summary(&store, &catalog, now_millis()) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "session_failed", "message": err.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct NextBody {
    pub mode: String,
    #[serde(default)]
    pub show_hint: bool,
    #[serde(default)]
    pub open_d2: bool,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub target_ladder: Option<String>,
}

pub async fn api_training_next(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Path(app_id): Path<String>,
    Json(body): Json<NextBody>,
) -> Response {
    let username = resolve_learner_id(principal);
    let Some(mode) = TrainingMode::parse(&body.mode) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_mode",
                "message": "mode must be char_to_code or radical_key"
            })),
        )
            .into_response();
    };
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let catalog = match load_catalog(workspace.as_path(), app_id.as_str()) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    let intent = body
        .intent
        .as_deref()
        .and_then(PracticeIntent::parse)
        .unwrap_or(PracticeIntent::Steady);
    let target_ladder = body
        .target_ladder
        .as_deref()
        .and_then(LadderStage::parse);
    let store = LearnerStore::open(workspace.as_path(), app_id.as_str(), username.as_str());
    let req = NextRequest {
        mode,
        show_hint: body.show_hint,
        open_d2: body.open_d2,
        intent,
        pack_id: body.pack_id,
        target_ladder,
    };
    match next_item(&store, &catalog, &req, now_millis()) {
        Ok(Ok(item)) => Json(item).into_response(),
        Ok(Err(empty)) => Json(empty).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "next_failed", "message": err.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ReviewBody {
    pub mode: String,
    pub item_id: String,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub correct: Option<bool>,
    #[serde(default)]
    pub latency_ms: u64,
    /// Optional fluency threshold (ms). Correct-but-slow → Hard when set.
    #[serde(default)]
    pub time_target_ms: Option<u64>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub target_ladder: Option<String>,
}

pub async fn api_training_review(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Path(app_id): Path<String>,
    Json(body): Json<ReviewBody>,
) -> Response {
    let username = resolve_learner_id(principal);
    let Some(mode) = TrainingMode::parse(&body.mode) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_mode",
                "message": "mode must be char_to_code or radical_key"
            })),
        )
            .into_response();
    };
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let catalog = match load_catalog(workspace.as_path(), app_id.as_str()) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    let intent = body
        .intent
        .as_deref()
        .and_then(PracticeIntent::parse)
        .unwrap_or(PracticeIntent::Steady);
    let target_ladder = body
        .target_ladder
        .as_deref()
        .and_then(LadderStage::parse);
    let store = LearnerStore::open(workspace.as_path(), app_id.as_str(), username.as_str());
    let req = ReviewRequest {
        mode,
        item_id: body.item_id,
        answer: body.answer,
        correct: body.correct,
        latency_ms: body.latency_ms,
        time_target_ms: body.time_target_ms,
        intent,
        pack_id: body.pack_id,
        target_ladder,
    };
    match review_item(&store, &catalog, &req, now_millis()) {
        Ok(result) => Json(result).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "review_failed", "message": err.to_string() })),
        )
            .into_response(),
    }
}

/// Invalidate cached catalog after bundle regeneration (optional ops hook).
#[allow(dead_code)]
pub fn invalidate_training_catalog(app_id: &str) {
    catalog_cache()
        .lock()
        .expect("catalog cache")
        .remove(app_id);
}
