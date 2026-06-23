use std::path::PathBuf;
use std::time::Instant;

use axum::{
    http::StatusCode,
    http::{HeaderName, HeaderValue},
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{HostAccountView, TopbarMenuContext, UiRouteMode, UploadFileEntry};
use mei_lang_kernel::{CompileOptions, WorkspaceAppMeta};

use crate::AppState;

use crate::http::compile_cache::{
    compile_app_with_cache, load_compile_artifact_only, CompileWithCacheOutcome,
};
use crate::http::host_api;
use crate::http::host_error_page::{self, HostShellAction};
use crate::http::pages::app::compiling_shell::{
    compile_bootstrap_probe_requested, compile_bootstrap_route_supported,
};
use crate::http::pages::app::query::AppQuery;

pub(super) enum CompileResolution {
    Outcome(CompileWithCacheOutcome),
    EarlyResponse(Response),
}

pub(super) fn maybe_handle_compile_bootstrap_probe(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    compile_options: &CompileOptions,
    components_root: &std::path::Path,
    _access_path_scene: Option<&str>,
) -> Option<Response> {
    if !compile_bootstrap_route_supported(route_mode) {
        return None;
    }
    if !compile_bootstrap_probe_requested(query) {
        return None;
    }
    let ready =
        load_compile_artifact_only(state, app_id, compile_options, components_root).is_some();
    Some(compile_bootstrap_probe_response(
        ready,
        if ready {
            "artifact_ready"
        } else {
            "artifact_missing"
        },
    ))
}

fn compile_bootstrap_probe_response(ready: bool, reason: &str) -> Response {
    let mut response = if ready {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::ACCEPTED.into_response()
    };
    if let Ok(value) = HeaderValue::from_str(if ready { "1" } else { "0" }) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-compile-bootstrap-ready"),
            value,
        );
    }
    if let Ok(value) = HeaderValue::from_str(reason) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-compile-bootstrap-reason"),
            value,
        );
    }
    response
}

pub(super) fn resolve_compile_outcome(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    _query: &AppQuery,
    compile_options: CompileOptions,
    components_root: PathBuf,
    access_path_scene: Option<&str>,
    manage_file: Option<&str>,
    _apps: &[WorkspaceAppMeta],
    _topbar_menus: &TopbarMenuContext,
    _normalized_preview_target: Option<&str>,
    _chrome_hidden: bool,
    _upload_enabled: bool,
    _upload_root_label: &str,
    _upload_files: &[UploadFileEntry],
    _auth_enabled: bool,
    _account_view: Option<&HostAccountView>,
    _discover_ms: u64,
    _app_started: Instant,
) -> CompileResolution {
    let compile_outcome =
        load_compile_artifact_only(state, app_id, &compile_options, components_root.as_path())
            .or_else(|| {
                let scene_hint = compile_options
                    .scene
                    .as_deref()
                    .or(access_path_scene)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?;
                if compile_options
                    .preview_target
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty())
                {
                    return None;
                }
                let target_hint = host_api::access_scene_target_hint(app_id, scene_hint)?;
                let fallback_options = CompileOptions {
                    scene: Some(scene_hint.to_string()),
                    preview_target: Some(target_hint),
                };
                load_compile_artifact_only(
                    state,
                    app_id,
                    &fallback_options,
                    components_root.as_path(),
                )
            });
    match compile_outcome {
        Some(outcome) => CompileResolution::Outcome(outcome),
        None if route_mode == UiRouteMode::Build => {
            match compile_app_with_cache(state, app_id, &compile_options, components_root.as_path())
            {
                Ok(outcome) => CompileResolution::Outcome(outcome),
                Err(_) => CompileResolution::EarlyResponse(
                    Redirect::temporary(&build_source_fallback_location(app_id)).into_response(),
                ),
            }
        }
        None => CompileResolution::EarlyResponse(render_access_artifact_unavailable(
            route_mode,
            app_id,
            compile_options.scene.as_deref().or(access_path_scene),
            manage_file,
        )),
    }
}

fn build_source_fallback_location(app_id: &str) -> String {
    format!("/apps/build/{app_id}?tab=overview")
}

fn render_access_artifact_unavailable(
    route_mode: UiRouteMode,
    app_id: &str,
    scene_hint: Option<&str>,
    manage_file: Option<&str>,
) -> Response {
    let mut actions = vec![HostShellAction {
        href: "/".to_string(),
        label: "返回首页".to_string(),
        primary: true,
    }];
    if let Some(scene_id) = scene_hint.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/app/{app_id}/scene/{scene_id}?chrome=none"),
                label: "重试当前场景".to_string(),
                primary: false,
            },
        );
    } else if let Some(target) = manage_file.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/build/{app_id}?file={target}"),
                label: "打开构建视图".to_string(),
                primary: false,
            },
        );
    }
    let gate = host_api::artifact_gate_status(app_id, scene_hint, manage_file);
    let message = gate.last_error.as_deref().map_or_else(
        || {
            "当前 access-only 宿主已切到 artifact-first 主路径，请先等待后台构建完成，或在构建视图中手动触发重建。"
                .to_string()
        },
        |error| {
            format!(
                "当前访问请求命中了未就绪产物；宿主不会因访问触发编译。最近一次构建状态：{error}"
            )
        },
    );
    let detail = scene_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|scene_id| {
            format!(
                "mode={} app={app_id} scene={scene_id} host_phase={} app_phase={} scope_phase={}",
                route_mode.slug(),
                gate.host_phase,
                gate.app_phase.as_deref().unwrap_or("missing"),
                gate.scope_phase.as_deref().unwrap_or("missing"),
            )
        })
        .unwrap_or_else(|| {
            format!(
                "mode={} app={app_id} host_phase={} app_phase={} scope_phase={}",
                route_mode.slug(),
                gate.host_phase,
                gate.app_phase.as_deref().unwrap_or("missing"),
                gate.scope_phase.as_deref().unwrap_or("missing"),
            )
        });
    let html = host_error_page::render_error_page(
        StatusCode::SERVICE_UNAVAILABLE,
        "访问态产物尚未就绪",
        message.as_str(),
        Some(detail.as_str()),
        &actions,
    );
    let mut response = Html(html).into_response();
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        HeaderName::from_static("retry-after"),
        HeaderValue::from_static("3"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::build_source_fallback_location;

    #[test]
    fn build_source_fallback_uses_overview_tab() {
        assert_eq!(
            build_source_fallback_location("zhifa"),
            "/apps/build/zhifa?tab=overview"
        );
    }
}
