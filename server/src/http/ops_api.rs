//! 受限运维面：只读写 ops registry / journal，不写 `.mei`。

use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use mei_lang_app::{scene_theme_style_for_theme_id, scene_viewport_theme_style};
use mei_lang_kernel::{
    apply_ops_patch_with_journal, compile_app_from_root_with_options, decode_theme_ref_token,
    journal_path, load_mei_config_for_app, ops_themes_revision_digest,
    resolve_app_root as kernel_resolve_app_root, resolve_components_root,
    resolve_default_scene_from_root, resolve_mei_config_path, CompileOptions, MeiConfig,
    OpsConfigPatch, OpsJournal,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::compile_cache::{
    compile_outcome_from_shared, resolve_runtime_compile_shared, RuntimeAccessPolicies,
};
use crate::http::pages::clear_page_render_cache;
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
    let themes_changed = body.patch.themes.is_some();
    match apply_ops_patch_with_journal(&app_root, &config_path, actor, summary, &body.patch) {
        Ok((config, entry)) => {
            let page_render_cache_cleared = if themes_changed {
                clear_page_render_cache()
            } else {
                0
            };
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "revision": entry.revision,
                    "config": config,
                    "page_render_cache_cleared": page_render_cache_cleared,
                })),
            )
                .into_response()
        }
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

#[derive(Debug, Deserialize)]
pub(crate) struct OpsThemeStyleQuery {
    #[serde(default)]
    scene: Option<String>,
    #[serde(default, rename = "themeId")]
    theme_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpsThemeStyleResponse {
    css_vars_style: String,
    theme_id: String,
    revision: String,
}

pub async fn ops_theme_style_get(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
    Query(query): Query<OpsThemeStyleQuery>,
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
    let revision = ops_themes_revision_digest(&config);
    let theme_revision_header = revision.clone();
    let scene_id = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| resolve_default_scene_from_root(&app_root).ok().flatten());
    let compile_options = CompileOptions {
        scene: scene_id.clone(),
        preview_target: None,
        ..Default::default()
    };
    let components_root = resolve_components_root(&state.source_root);
    let (css_vars_style, theme_id) =
        if let Ok(Some(resolution)) = resolve_runtime_compile_shared(
            &state,
            &app_id,
            &compile_options,
            components_root.as_path(),
            RuntimeAccessPolicies::default_for_access_host(),
            mei_lang_app::UiRouteMode::App,
        ) {
            let outcome = compile_outcome_from_shared(resolution.outcome);
            let theme_id = outcome
                .compiled
                .scene_contract
                .as_ref()
                .map(theme_id_from_scene_contract)
                .unwrap_or_else(|| "page".to_string());
            (
                scene_viewport_theme_style(&outcome.compiled, Some(&config)),
                theme_id,
            )
        } else if let Ok(compiled) =
            compile_app_from_root_with_options(source_root, &app_root, compile_options)
        {
            let theme_id = compiled
                .scene_contract
                .as_ref()
                .map(theme_id_from_scene_contract)
                .unwrap_or_else(|| "page".to_string());
            (
                scene_viewport_theme_style(&compiled, Some(&config)),
                theme_id,
            )
        } else if let Some(theme_id) = query
            .theme_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            (
                scene_theme_style_for_theme_id(theme_id, Some(&config)),
                theme_id.to_string(),
            )
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "compile artifact missing and no themeId provided"})),
            )
                .into_response();
        };
    let mut response = (
        StatusCode::OK,
        Json(OpsThemeStyleResponse {
            css_vars_style,
            theme_id,
            revision,
        }),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(theme_revision_header.as_str()) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-theme-revision"),
            value,
        );
    }
    response
}

fn theme_id_from_scene_contract(
    contract: &mei_lang_kernel::SceneContract,
) -> String {
    contract
        .scene
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .or_else(|| contract.scene.theme.clone())
        .or_else(|| contract.scene.profile.clone())
        .unwrap_or_else(|| "page".to_string())
}
