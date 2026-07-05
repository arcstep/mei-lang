//! Permanent redirects from legacy host-shell routes.

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

pub async fn redirect_apps_upload(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let mut target = format!("/upload?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_config(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let mut target = format!("/config?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_runtime(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    let mut target = format!("/runtime?app={}", encode_query_component(app_id.trim()));
    if let Some(query) = uri.0.query() {
        if !query.is_empty() {
            target.push('&');
            target.push_str(query);
        }
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_access(Path(app_id): Path<String>) -> Response {
    let target = format!(
        "/apps/{}/view?surface=app",
        encode_query_component(app_id.trim())
    );
    Redirect::permanent(target.as_str()).into_response()
}

fn append_query(target: &mut String, query: &str) {
    if query.is_empty() {
        return;
    }
    if target.contains('?') {
        target.push('&');
    } else {
        target.push('?');
    }
    target.push_str(query);
}

/// View canonical query: only `scene` and `chrome` (surface comes from redirect target).
fn canonical_view_query_from_uri(uri: &Uri) -> String {
    let Some(query) = uri.query() else {
        return String::new();
    };
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            match key {
                "scene" | "chrome" => Some(format!("{key}={value}")),
                _ => None,
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn append_canonical_view_query(target: &mut String, uri: &Uri) {
    let filtered = canonical_view_query_from_uri(uri);
    if !filtered.is_empty() {
        append_query(target, filtered.as_str());
    }
}

pub async fn redirect_apps_surface_to_view(
    Path((app_id, surface)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    let app_id = app_id.trim();
    let surface = surface.trim();
    let mut target = format!(
        "/apps/{}/view?surface={}",
        encode_query_component(app_id),
        encode_query_component(surface)
    );
    if let Some(query) = uri.0.query() {
        let _ = query;
        append_canonical_view_query(&mut target, &uri.0);
    }
    Redirect::permanent(target.as_str()).into_response()
}

pub async fn redirect_apps_app_to_view(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    redirect_apps_surface_to_view(Path((app_id, "app".to_string())), uri).await
}

pub async fn redirect_apps_layout_to_view(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    redirect_apps_surface_to_view(Path((app_id, "layout".to_string())), uri).await
}

pub async fn redirect_apps_prototype_to_view(
    Path(app_id): Path<String>,
    uri: OriginalUri,
) -> Response {
    redirect_apps_surface_to_view(Path((app_id, "prototype".to_string())), uri).await
}

pub async fn redirect_apps_app_scene(
    Path((app_id, scene)): Path<(String, String)>,
    uri: OriginalUri,
) -> Response {
    let mut target = format!(
        "/apps/{}/view?surface=app&scene={}",
        encode_query_component(app_id.trim()),
        encode_query_component(scene.trim())
    );
    if let Some(query) = uri.0.query() {
        let _ = query;
        append_canonical_view_query(&mut target, &uri.0);
    }
    Redirect::permanent(target.as_str()).into_response()
}
