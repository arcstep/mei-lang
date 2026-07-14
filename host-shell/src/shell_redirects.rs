//! Permanent redirects for Host chrome paths; legacy Scene-as-route surfaces return 410.
//!
//! Canonical Access URL: `/apps/{app_id}/{stage_id}` (default stage from app.mei `default_stage`).

use axum::{
    extract::{OriginalUri, Path},
    http::{StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde_json::json;

fn encode_query_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            _ => format!("%{:02X}", ch as u32),
        })
        .collect()
}

fn redirect_with_query(target: &str, uri: &Uri) -> Response {
    let location = match uri.query() {
        Some(query) if !query.is_empty() => format!("{target}?{query}"),
        _ => target.to_string(),
    };
    Redirect::permanent(location.as_str()).into_response()
}

pub async fn redirect_root_to_home(uri: OriginalUri) -> Response {
    redirect_with_query("/home", &uri.0)
}

pub async fn redirect_host_upload(uri: OriginalUri) -> Response {
    redirect_with_query("/upload", &uri.0)
}

pub async fn redirect_host_config(uri: OriginalUri) -> Response {
    redirect_with_query("/config", &uri.0)
}

pub async fn redirect_host_runtime(uri: OriginalUri) -> Response {
    redirect_with_query("/runtime", &uri.0)
}

pub async fn redirect_apps_upload(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let mut target = format!("/upload?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_config(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let mut target = format!("/config?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_runtime(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let mut target = format!("/runtime?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

/// Canonical Access stage href.
pub fn access_stage_path(app_id: &str, stage_id: &str) -> String {
    let app = encode_query_component(app_id.trim());
    let stage = stage_id.trim();
    let stage = if stage.is_empty() { "home" } else { stage };
    format!("/apps/{app}/{}", encode_query_component(stage))
}

fn query_param(uri: &Uri, key: &str) -> Option<String> {
    uri.query().and_then(|query| {
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == key && !v.is_empty() {
                Some(
                    urlencoding_decode(v)
                        .unwrap_or_else(|| v.to_string())
                        .trim()
                        .to_string(),
                )
            } else {
                None
            }
        })
    })
}

fn urlencoding_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = from_hex(bytes[i + 1])?;
                let lo = from_hex(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn gone_legacy_surface(app_id: &str, surface: &str, hinted_stage: &str) -> Response {
    let canonical = access_stage_path(app_id, hinted_stage);
    (
        StatusCode::GONE,
        Json(json!({
            "error": "legacy app surface removed",
            "code": "legacy_surface_gone",
            "appId": app_id,
            "surface": surface,
            "hint": format!(
                "Use canonical Access Stage URL `{canonical}` (Phase 9 removed /apps/{{app}}/{surface} redirects)"
            ),
            "canonical": canonical,
        })),
    )
        .into_response()
}

pub async fn redirect_apps_access(Path(app_id): Path<String>) -> Response {
    gone_legacy_surface(app_id.trim(), "access", "home")
}

/// `/apps/{id}/view?...` — Phase 9: 410 Gone (no silent Stage rewrite).
pub async fn redirect_apps_view_to_stage(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let scene = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    gone_legacy_surface(app_id.trim(), "view", scene.as_str())
}

pub async fn redirect_apps_app_to_stage(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    gone_legacy_surface(app_id.trim(), "app", stage.as_str())
}

pub async fn redirect_apps_app_scene(
    Path((app_id, scene)): Path<(String, String)>,
    _uri: OriginalUri,
) -> Response {
    let stage = if scene == "scene" {
        "home".to_string()
    } else if let Some(rest) = scene.strip_prefix("scene/") {
        rest.split('/').next().unwrap_or("home").to_string()
    } else {
        scene
    };
    gone_legacy_surface(app_id.trim(), "app/scene", stage.trim())
}

/// Dedicated handler for `/apps/:app_id/app/scene/:scene`.
pub async fn redirect_apps_app_scene_id(
    Path((app_id, scene)): Path<(String, String)>,
    _uri: OriginalUri,
) -> Response {
    gone_legacy_surface(app_id.trim(), "app/scene", scene.trim())
}

/// Mode-first legacy: `/apps/app/{app_id}`
pub async fn redirect_mode_first_app_root(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    gone_legacy_surface(app_id.trim(), "apps/app", stage.as_str())
}

/// Mode-first legacy: `/apps/app/{app_id}/scene/{scene}`
pub async fn redirect_mode_first_app_scene(
    Path((app_id, scene)): Path<(String, String)>,
    _uri: OriginalUri,
) -> Response {
    gone_legacy_surface(app_id.trim(), "apps/app/scene", scene.trim())
}

/// Mode-first legacy: `/apps/app/{app_id}/*tail`
pub async fn redirect_mode_first_app_tail(
    Path((app_id, tail)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    let stage = if let Some(rest) = tail.strip_prefix("scene/") {
        rest.split('/').next().unwrap_or("home").to_string()
    } else {
        query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string())
    };
    gone_legacy_surface(app_id.trim(), "apps/app/*", stage.trim())
}

pub async fn redirect_apps_layout_to_stage(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    gone_legacy_surface(app_id.trim(), "layout", stage.as_str())
}

pub async fn redirect_apps_prototype_to_stage(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    gone_legacy_surface(app_id.trim(), "prototype", stage.as_str())
}

/// Reserved second-path segments that are not Access stages.
pub fn is_reserved_stage_segment(segment: &str) -> bool {
    matches!(
        segment.trim().to_ascii_lowercase().as_str(),
        "view"
            | "layout"
            | "prototype"
            | "app"
            | "access"
            | "access-only"
            | "access_only"
            | "build"
            | "manage"
            | "run"
            | "copilot"
            | "presentation"
            | "speaker"
            | "slides"
            | "upload"
            | "config"
            | "runtime"
            | "~"
    )
}
