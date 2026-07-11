//! Permanent redirects from legacy host-shell routes to Access stage paths.
//!
//! Canonical Access URL: `/apps/{app_id}/{stage_id}` (default stage `home`).

use axum::{
    extract::{OriginalUri, Path},
    http::Uri,
    response::{IntoResponse, Redirect, Response},
};

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

fn append_chrome_query(target: &mut String, uri: &Uri) {
    if let Some(chrome) = query_param(uri, "chrome") {
        if chrome != "full" {
            if target.contains('?') {
                target.push('&');
            } else {
                target.push('?');
            }
            target.push_str("chrome=");
            target.push_str(&encode_query_component(&chrome));
        }
    }
}

fn redirect_to_stage(app_id: &str, stage: &str, uri: &Uri) -> Response {
    let mut target = access_stage_path(app_id, stage);
    append_chrome_query(&mut target, uri);
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_access(Path(app_id): Path<String>) -> Response {
    Redirect::permanent(access_stage_path(app_id.trim(), "home").as_str()).into_response()
}

/// `/apps/{id}/view?surface=*&scene=*` → `/apps/{id}/{stage}`
/// layout/prototype surfaces seal to Access default stage.
pub async fn redirect_apps_view_to_stage(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let surface = query_param(&uri.0, "surface")
        .unwrap_or_else(|| "app".to_string())
        .to_ascii_lowercase();
    let scene = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    // 布局/原型产品面已封口：一律落到 Access 舞台
    let stage = if matches!(surface.as_str(), "layout" | "prototype" | "build" | "manage") {
        "home"
    } else {
        scene.as_str()
    };
    // view/scene/{id} path tail
    let path = uri.0.path();
    let stage = path
        .strip_prefix(&format!("/apps/{}/view/scene/", app_id.trim()))
        .map(|rest| rest.split('/').next().unwrap_or(stage))
        .unwrap_or(stage);
    redirect_to_stage(app_id.trim(), stage, &uri.0)
}

pub async fn redirect_apps_app_to_stage(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    redirect_to_stage(app_id.trim(), &stage, &uri.0)
}

pub async fn redirect_apps_app_scene(
    Path((app_id, scene)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    // `/apps/{id}/app/scene/{scene}` or catch-all tail — Path may be (app, rest)
    let stage = if scene == "scene" {
        // unlikely; handled by *tail route
        query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string())
    } else if let Some(rest) = scene.strip_prefix("scene/") {
        rest.split('/').next().unwrap_or("home").to_string()
    } else {
        scene
    };
    redirect_to_stage(app_id.trim(), stage.trim(), &uri.0)
}

/// Dedicated handler for `/apps/:app_id/app/scene/:scene`.
pub async fn redirect_apps_app_scene_id(
    Path((app_id, scene)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    redirect_to_stage(app_id.trim(), scene.trim(), &uri.0)
}

/// Mode-first legacy: `/apps/app/{app_id}` → `/apps/{app_id}/{stage}`
pub async fn redirect_mode_first_app_root(Path(app_id): Path<String>, uri: OriginalUri) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    redirect_to_stage(app_id.trim(), &stage, &uri.0)
}

/// Mode-first legacy: `/apps/app/{app_id}/scene/{scene}`
pub async fn redirect_mode_first_app_scene(
    Path((app_id, scene)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    redirect_to_stage(app_id.trim(), scene.trim(), &uri.0)
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
    redirect_to_stage(app_id.trim(), stage.trim(), &uri.0)
}

pub async fn redirect_apps_layout_to_stage(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    redirect_to_stage(app_id.trim(), &stage, &uri.0)
}

pub async fn redirect_apps_prototype_to_stage(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let stage = query_param(&uri.0, "scene").unwrap_or_else(|| "home".to_string());
    redirect_to_stage(app_id.trim(), &stage, &uri.0)
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
    )
}
