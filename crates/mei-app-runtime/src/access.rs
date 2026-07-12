//! Access thin-shell HTML (minimal; full SSR chrome remains host-injectable).

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::resolve_default_scene_from_root;
use serde::Deserialize;

use crate::host_data::inject_view_revision_envelope;
use crate::state::SharedRuntimeState;

/// Floating FAB so Access clients can attach Copilot chrome (mirrors host-shell thin shell).
const THIN_SHELL_ACCESS_FAB_HTML: &str = concat!(
    r#"<div id="access-chat-floating-root" class="access-chat-floating-root" data-open="false" data-mei-stage-kind="scene" data-mei-fab-policy="required">"#,
    r#"<button id="access-chat-fab" class="access-chat-fab" type="button" aria-label="展开 Copilot 工具条" title="展开 Copilot 工具条" data-mei-fab-policy="required">"#,
    r#"<img class="access-chat-fab-icon" src="/app-assets/favicon.svg" alt="" />"#,
    r#"</button></div>"#,
);

#[derive(Debug, Deserialize, Default)]
pub struct AccessQuery {
    /// Reserved for host chrome injection (`none` | `full`); Access thin shell ignores for now.
    #[serde(default)]
    pub chrome: Option<String>,
}

pub fn thin_access_shell_document(app_id: &str, scene_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"/><title>{app_id}</title><meta name="mei-view" content="app"/><link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/><link rel="stylesheet" href="/app-bundles/styles.css"/></head><body class="app-view sl-theme-dark" data-mei-view="app" data-app-id="{app_id}" data-scene-id="{scene_id}" data-mei-app-runtime="1"><div class="shell shell-surface scene-shell mei-text-primary min-h-screen flex min-h-0 flex-col" id="mei-compose-host" data-scene="{scene_id}"><div id="mei-host-topbar-slot" data-mei-host-chrome="top"></div><main class="main flex min-h-0 flex-1 flex-col overflow-hidden"><div class="preview-pane-scroll shell-inner min-h-0 flex-1 overflow-auto" id="mei-compose-root" data-scene="{scene_id}" data-mei-compose-placeholder="1" aria-busy="true"></div><div id="mei-thin-shell-fallback" class="mei-thin-shell-fallback mei-p-4 mei-text-muted hidden" role="status" hidden>正在加载场景内容…</div></main><div id="mei-host-statusbar-slot" data-mei-host-chrome="bottom"></div></div>{fab}<script defer src="/app-bundles/access.js"></script></body></html>"#,
        app_id = app_id,
        scene_id = scene_id,
        fab = THIN_SHELL_ACCESS_FAB_HTML,
    )
}

fn resolve_default_scene(state: &SharedRuntimeState) -> String {
    let app_root = state.host.app_root();
    resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "home".to_string())
}

fn html_response(html: String) -> Response {
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-thin-shell"),
        HeaderValue::from_static("1"),
    );
    response
}

pub async fn access_app_root(
    State(state): State<SharedRuntimeState>,
    Path(app_id): Path<String>,
    Query(query): Query<AccessQuery>,
    _headers: HeaderMap,
) -> Response {
    if app_id != state.app_id() {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    let _chrome = query.chrome.as_deref();
    let scene_id = resolve_default_scene(&state);
    let html = thin_access_shell_document(app_id.as_str(), scene_id.as_str());
    let html = inject_view_revision_envelope(html, app_id.as_str(), scene_id.as_str(), "app");
    html_response(html)
}

pub async fn access_app_stage(
    State(state): State<SharedRuntimeState>,
    Path((app_id, stage)): Path<(String, String)>,
    Query(query): Query<AccessQuery>,
    _headers: HeaderMap,
) -> Response {
    if app_id != state.app_id() {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    let _chrome = query.chrome.as_deref();
    let route = UiRouteMode::from_slug(stage.as_str());
    let scene_id = if matches!(
        stage.as_str(),
        "app" | "access" | "view" | "layout" | "prototype"
    ) {
        resolve_default_scene(&state)
    } else {
        stage.clone()
    };
    let surface = if matches!(
        stage.as_str(),
        "app" | "access" | "view" | "layout" | "prototype"
    ) {
        route.slug()
    } else {
        "app"
    };
    let html = thin_access_shell_document(app_id.as_str(), scene_id.as_str());
    let html = inject_view_revision_envelope(html, app_id.as_str(), scene_id.as_str(), surface);
    html_response(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thin_shell_contains_compose_host_and_envelope_hooks() {
        let html = thin_access_shell_document("demo", "home");
        assert!(html.contains("mei-compose-host"));
        assert!(html.contains("data-app-id=\"demo\""));
        assert!(html.contains("data-scene-id=\"home\""));
        assert!(html.contains("access-chat-fab"));
        let injected = inject_view_revision_envelope(html, "demo", "home", "app");
        assert!(injected.contains("view_revision_envelope"));
        assert!(injected.contains("thin_shell"));
    }
}
