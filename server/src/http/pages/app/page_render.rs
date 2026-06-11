use std::{fs, path::Path};

use axum::{
    http::{HeaderName, HeaderValue},
    response::Response,
};
use mei_lang_app::{HostAccountView, UploadFileEntry};
use mei_lang_kernel::{
    load_mei_config_for_app, resolve_default_scene_from_root, CompiledApp, HostSurface,
    WorkspaceAppMeta,
};

use super::super::super::compile_cache::CompileWithCacheOutcome;
use crate::auth::AuthPrincipal;

pub(super) fn html_escape_min(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn account_view_for_principal(
    principal: Option<&AuthPrincipal>,
) -> Option<HostAccountView> {
    principal.map(|principal| HostAccountView {
        logged_in: true,
        username: principal.username.clone(),
        profile: principal.profile.clone(),
        role: principal.role_slug().to_string(),
        capabilities: principal.capabilities(),
    })
}

fn diagnostic_message_by_code(compiled: &CompiledApp, code: &str) -> Option<String> {
    compiled
        .diagnostics
        .iter()
        .find(|diag| diag.code == code)
        .map(|diag| diag.message.clone())
}

pub(super) fn insert_manage_compile_observability_headers(
    res: &mut Response,
    compiled: &CompiledApp,
) {
    for (header, code) in [
        (
            "x-mei-compile-optimization-status",
            "compile_optimization_status",
        ),
        ("x-mei-compile-stage-timing", "compile_stage_timing"),
        ("x-mei-compile-cache-stats", "compile_cache_stats"),
        ("x-mei-dependency-graph-stats", "dependency_graph_stats"),
        ("x-mei-catalog-compile-stats", "catalog_compile_stats"),
        ("x-mei-catalog-filter-stats", "catalog_filter_stats"),
    ] {
        let Some(message) = diagnostic_message_by_code(compiled, code) else {
            continue;
        };
        if let Ok(value) = HeaderValue::from_str(&message) {
            res.headers_mut()
                .insert(HeaderName::from_static(header), value);
        }
    }
}

pub(super) fn insert_manage_compile_request_headers(
    res: &mut Response,
    outcome: &CompileWithCacheOutcome,
) {
    for (header, value) in [
        (
            "x-mei-compile-cache-hit",
            if outcome.cache_hit { "1" } else { "0" }.to_string(),
        ),
        (
            "x-mei-compile-revision-scope",
            outcome.revision_scope.clone(),
        ),
        (
            "x-mei-compile-cache-validation",
            outcome.cache_validation.clone(),
        ),
        ("x-mei-compile-ms", outcome.compile_ms.to_string()),
        (
            "x-mei-compile-cache-lookup-ms",
            outcome.cache_lookup_ms.to_string(),
        ),
    ] {
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            res.headers_mut()
                .insert(HeaderName::from_static(header), header_value);
        }
    }
}

pub(super) fn insert_page_render_cache_hit_header(res: &mut Response, cache_hit: bool) {
    let value = if cache_hit { "1" } else { "0" };
    if let Ok(header_value) = HeaderValue::from_str(value) {
        res.headers_mut().insert(
            HeaderName::from_static("x-mei-page-render-cache-hit"),
            header_value,
        );
    }
}
pub(super) fn upload_rel_from_config(app_root: &Path, source_root: &Path) -> Option<String> {
    let config = load_mei_config_for_app(app_root, Some(source_root));
    config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
}

pub(super) fn list_upload_files(upload_root: &Path, _upload_rel: &str) -> Vec<UploadFileEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(upload_root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        out.push(UploadFileEntry {
            path: name.clone(),
            name,
            is_dir: file_type.is_dir(),
        });
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    out
}

pub(super) fn app_title_for(apps: &[WorkspaceAppMeta], app_id: &str) -> String {
    apps.iter()
        .find(|app| app.id == app_id)
        .map(|app| app.title.clone())
        .unwrap_or_else(|| app_id.to_string())
}

pub(super) fn lightweight_access_scene(
    app_root: &Path,
    query_scene: Option<&str>,
) -> Option<String> {
    query_scene
        .map(str::trim)
        .filter(|scene| !scene.is_empty())
        .map(str::to_string)
        .or_else(|| resolve_default_scene_from_root(app_root).ok().flatten())
}

pub(super) fn access_only_surface_enabled() -> bool {
    std::env::var("MEI_HOST_SURFACE")
        .ok()
        .map(|value| HostSurface::from_host_surface_flag(&value))
        .is_some_and(|surface| surface == HostSurface::AccessOnlyHost)
}
