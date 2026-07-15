//! Access thin-shell HTML (minimal; full SSR chrome remains host-injectable).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::resolve_default_scene_from_root;
use serde::Deserialize;

use crate::host_data::{fill_runtime_asset_version, inject_view_revision_envelope_with_dev_eval};
use crate::state::{AppRuntimeServeState, SharedRuntimeState};

fn access_html_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn inject_html_before_head_close(html: String, fragment: &str) -> String {
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + fragment.len());
        out.push_str(&html[..pos]);
        out.push_str(fragment);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{fragment}{html}")
    }
}

/// Host workspace pages inject shell theme CSS vars on `<body>`; Runtime thin shell
/// must do the same or topbar/status text falls back to black on a transparent chrome.
fn fill_runtime_page_theme(html: String, workspace_root: &std::path::Path) -> String {
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root);
    let style = mei_lang_app::shell_body_theme_style(&workspace);
    html.replace(
        "__MEI_PAGE_BODY_THEME_STYLE__",
        html_escape_attr(style.as_str()).as_str(),
    )
}

/// Host thin-shell injects scene component modules; Runtime must do the same or
/// custom elements (`mei-text`, `mei-map-maplibre`, …) never upgrade.
fn inject_runtime_component_scripts(
    html: String,
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> String {
    let assets = mei_host_graph::build_runtime_plans_document(workspace_root, app_id, scene_id, "")
        .map(|doc| doc.component_assets)
        .unwrap_or_default();
    if assets.is_empty() {
        return html;
    }
    let mut scripts = String::new();
    for asset in assets {
        let script = asset.script.trim();
        if script.is_empty() {
            continue;
        }
        let src = html_escape_attr(&format!("/workspace-components/{script}"));
        scripts.push_str(&format!(
            r#"<script type="module" src="{src}" data-mei-component-asset="true"></script>"#
        ));
    }
    if scripts.is_empty() {
        return html;
    }
    inject_html_before_head_close(html, scripts.as_str())
}

/// Host SSR embeds `mei-host-runtime-capabilities`; without it metric/rows query stay disabled
/// (`runtime_capabilities: {}` → `mei-text` stuck on `--`).
fn inject_runtime_capabilities(html: String, app_id: &str) -> String {
    let payload = serde_json::json!({
        "data_mode": "eval",
        "rows_query": {
            "enabled": true,
            "api": format!("/api/datasets/query/{app_id}"),
            "scene_qualified": true,
        },
        "fixture_query": {
            "enabled": false,
            "api": format!("/api/datasets/fixture/{app_id}"),
            "scene_qualified": true,
        },
        "metric_query": {
            "enabled": true,
            "api": format!("/api/datasets/metrics/{app_id}"),
            "scene_qualified": true,
        },
        "metric_batch_query": {
            "enabled": true,
            "api": format!("/api/datasets/metrics/{app_id}"),
            "scene_qualified": true,
        },
        "static_display": { "enabled": false },
    });
    let script = format!(
        r#"<script id="mei-host-runtime-capabilities" type="application/json">{payload}</script>"#,
        payload = payload
    );
    inject_html_before_head_close(html, script.as_str())
}

fn finalize_access_html(
    state: &AppRuntimeServeState,
    app_id: &str,
    scene_id: &str,
    surface: &str,
) -> String {
    let cache_key = format!(
        "{}|{}|{}|{}",
        state.host.workspace_root.display(),
        app_id,
        scene_id,
        surface
    );
    if let Ok(cache) = access_html_cache().lock() {
        if let Some(html) = cache.get(cache_key.as_str()) {
            return html.clone();
        }
    }
    let html = thin_access_shell_document(app_id, scene_id);
    let html = fill_runtime_page_theme(html, state.host.workspace_root.as_path());
    let dev_eval = mei_lang_kernel::RuntimeDevEvalGate::from_runtime_plan(
        state.spec.config_snapshot.runtime_plan.clone(),
        app_id,
    )
    .client_payload();
    let bootstrap_status = mei_host_graph::bootstrap_embed_status(
        state.host.workspace_root.as_path(),
        app_id,
        scene_id,
    );
    let client_revision = bootstrap_status
        .allowed
        .then_some(bootstrap_status.client_revision)
        .flatten();
    let html = inject_view_revision_envelope_with_dev_eval(
        html,
        app_id,
        scene_id,
        surface,
        Some(&dev_eval),
        client_revision.as_deref(),
    );
    let html = inject_runtime_capabilities(html, app_id);
    let html = inject_runtime_component_scripts(
        html,
        state.host.workspace_root.as_path(),
        app_id,
        scene_id,
    );
    let html = fill_runtime_asset_version(html);
    if let Ok(mut cache) = access_html_cache().lock() {
        cache.insert(cache_key, html.clone());
    }
    html
}

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
    #[serde(default)]
    pub scene: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
}

pub fn thin_access_shell_document(app_id: &str, scene_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"/><title>{app_id}</title><meta name="mei-view" content="app"/><link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/><link rel="stylesheet" href="/app-bundles/styles.css?v=__MEI_HOST_ASSET_VERSION__"/><link rel="stylesheet" href="/app-assets/host-shell.css"/><script defer src="/app-bundles/shoelace.js?v=__MEI_HOST_ASSET_VERSION__"></script></head><body class="app-view sl-theme-dark" style="__MEI_PAGE_BODY_THEME_STYLE__" data-mei-view="app" data-app-id="{app_id}" data-scene-id="{scene_id}" data-mei-app-runtime="1"><div class="shell shell-surface scene-shell mei-text-primary min-h-screen flex min-h-0 flex-col" id="mei-compose-host" data-scene="{scene_id}"><div id="mei-host-topbar-slot" data-mei-host-chrome="top"></div><main class="main flex min-h-0 flex-1 flex-col overflow-hidden"><div class="preview-pane-scroll shell-inner min-h-0 flex-1 overflow-auto" id="mei-compose-root" data-scene="{scene_id}" data-mei-compose-placeholder="1" aria-busy="true"></div><div id="mei-thin-shell-fallback" class="mei-thin-shell-fallback mei-p-4 mei-text-muted hidden" role="status" hidden>正在加载场景内容…</div></main><div id="mei-host-statusbar-slot" data-mei-host-chrome="bottom"></div></div>{fab}<script defer src="/app-bundles/access.js?v=__MEI_HOST_ASSET_VERSION__"></script></body></html>"#,
        app_id = app_id,
        scene_id = scene_id,
        fab = THIN_SHELL_ACCESS_FAB_HTML,
    )
}

fn resolve_default_scene(state: &AppRuntimeServeState) -> String {
    let app_root = state.host.app_root();
    resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "home".to_string())
}

pub(crate) fn prime_access_html(state: &AppRuntimeServeState) {
    let scene_id = resolve_default_scene(state);
    let _ = finalize_access_html(state, state.app_id(), scene_id.as_str(), "app");
}

fn html_response(html: String) -> Response {
    let mut response = Html(html).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
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
    let html = finalize_access_html(&state, app_id.as_str(), scene_id.as_str(), "app");
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
    let route = UiRouteMode::from_slug(stage.as_str());
    let is_surface_slug = matches!(
        stage.as_str(),
        "app" | "access" | "view" | "layout" | "prototype"
    );
    let scene_id = if is_surface_slug {
        resolve_default_scene(&state)
    } else {
        stage.clone()
    };
    if !is_surface_slug {
        let app_root = state.host.app_root();
        let declared = mei_lang_kernel::resolve_scene_ids_from_root(app_root.as_path())
            .ok()
            .unwrap_or_default();
        if !declared.is_empty() && !declared.iter().any(|scene| scene == scene_id.as_str()) {
            // Phase 9: no silent redirect to default_stage.
            return (
                StatusCode::NOT_FOUND,
                format!(
                    "stage `{scene_id}` not found; use a declared stage (default: {})",
                    resolve_default_scene(&state)
                ),
            )
                .into_response();
        }
    }
    let surface = if is_surface_slug {
        route.slug()
    } else {
        "app"
    };
    let mut html = finalize_access_html(&state, app_id.as_str(), scene_id.as_str(), surface);
    html = stamp_temp_stage_axes(html, &query);
    html_response(html)
}

fn stamp_temp_stage_axes(html: String, query: &AccessQuery) -> String {
    let mut attrs = String::new();
    if let Some(scope) = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        attrs.push_str(&format!(
            " data-mei-scope=\"{}\"",
            html_escape_attr(scope)
        ));
    }
    if let Some(focus) = query
        .focus
        .as_deref()
        .or(query.node.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        attrs.push_str(&format!(
            " data-mei-focus=\"{}\"",
            html_escape_attr(focus)
        ));
    }
    if attrs.is_empty() {
        return html;
    }
    if let Some(idx) = html.find("<body ") {
        let mut out = String::with_capacity(html.len() + attrs.len());
        out.push_str(&html[..idx + 6]);
        out.push_str(attrs.trim_start());
        out.push(' ');
        out.push_str(&html[idx + 6..]);
        return out;
    }
    html
}

/// Phase 8.5: `/apps/{app}/~/{scope_or_node}` temporary Stage thin shell.
/// Host resolves ScopeTarget and forwards `?scene=&scope=&focus=`; closure applies
/// via view-revision compose axes (same pipeline as full stage).
pub async fn access_app_temp_stage(
    State(state): State<SharedRuntimeState>,
    Path((app_id, target_tail)): Path<(String, String)>,
    Query(query): Query<AccessQuery>,
    headers: HeaderMap,
) -> Response {
    let stage = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| mei_host_graph::infer_stage_from_temp_target(target_tail.as_str()));
    access_app_stage(State(state), Path((app_id, stage)), Query(query), headers).await
}

/// Legacy `/apps/{app}/{stage}/{tier…}` — serve stage shell (Host redirects to `~/`).
pub async fn access_app_scoped_stage(
    State(state): State<SharedRuntimeState>,
    Path((app_id, stage, _scoped_tail)): Path<(String, String, String)>,
    Query(query): Query<AccessQuery>,
    headers: HeaderMap,
) -> Response {
    if stage.trim() == "~" {
        return access_app_temp_stage(
            State(state),
            Path((app_id, _scoped_tail)),
            Query(query),
            headers,
        )
        .await;
    }
    access_app_stage(State(state), Path((app_id, stage)), Query(query), headers).await
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
        assert!(html.contains("/app-bundles/access.js?v=__MEI_HOST_ASSET_VERSION__"));
        assert!(html.contains("/app-bundles/styles.css?v=__MEI_HOST_ASSET_VERSION__"));
        assert!(html.contains("/app-bundles/shoelace.js?v=__MEI_HOST_ASSET_VERSION__"));
        assert!(html.contains("/app-assets/host-shell.css"));
        assert!(html.contains("__MEI_PAGE_BODY_THEME_STYLE__"));
        let injected = fill_runtime_asset_version(
            crate::host_data::inject_view_revision_envelope_with_dev_eval(
                html,
                "demo",
                "home",
                "app",
                None,
                Some("client-rev-1"),
            ),
        );
        assert!(injected.contains("view_revision_envelope"));
        assert!(injected.contains("mei-bootstrap-client-revision"));
        assert!(injected.contains("client-rev-1"));
        assert!(injected.contains("thin_shell"));
        assert!(!injected.contains("__MEI_HOST_ASSET_VERSION__"));
        assert!(injected.contains("/app-bundles/access.js?v="));
    }
}
