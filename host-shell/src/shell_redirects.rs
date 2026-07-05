//! Permanent redirects from legacy host-shell routes.

use axum::{
    extract::{OriginalUri, Path},
    http::Uri,
    response::{IntoResponse, Redirect, Response},
};

use crate::pages::AppQuery;

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
        "/apps/{}/app",
        encode_query_component(app_id.trim())
    );
    Redirect::permanent(target.as_str()).into_response()
}

pub fn mcg_redirect_for_app(app_id: &str, query: &AppQuery) -> String {
    let mut target = format!("/mcg?app={}", encode_query_component(app_id.trim()));
    if let Some(file) = query.file.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        target.push_str("&file=");
        target.push_str(&encode_query_component(file));
    }
    target
}
