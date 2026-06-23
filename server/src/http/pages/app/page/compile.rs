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

use crate::http::compile_cache::{compile_app_with_cache, CompileWithCacheOutcome};
use crate::http::host_api;
use crate::http::host_error_page::{self, HostShellAction};
use crate::http::pages::app::compiling_shell::{
    compile_bootstrap_probe_requested, compile_bootstrap_route_supported,
};
use crate::http::pages::app::query::AppQuery;

pub(super) enum CompileResolution {
    Outcome(ResolvedCompileOutcome),
    EarlyResponse(Response),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompileFeedbackMetadata {
    pub path: &'static str,
    pub reason: &'static str,
    pub scope_kind: &'static str,
    pub diagnostic_error_count: usize,
}

pub(super) struct ResolvedCompileOutcome {
    pub outcome: CompileWithCacheOutcome,
    pub feedback: CompileFeedbackMetadata,
}

fn compile_feedback_scope_kind(options: &CompileOptions) -> &'static str {
    let has_scene = options
        .scene
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    match (has_scene, has_target) {
        (true, true) => "scene_target",
        (true, false) => "scene_only",
        (false, true) => "target_only",
        (false, false) => "full_app",
    }
}

fn compile_options_equal(left: &CompileOptions, right: &CompileOptions) -> bool {
    left.scene.as_deref().map(str::trim) == right.scene.as_deref().map(str::trim)
        && left.preview_target.as_deref().map(str::trim)
            == right.preview_target.as_deref().map(str::trim)
}

fn compile_feedback(
    path: &'static str,
    reason: &'static str,
    options: &CompileOptions,
    diagnostic_error_count: usize,
) -> CompileFeedbackMetadata {
    CompileFeedbackMetadata {
        path,
        reason,
        scope_kind: compile_feedback_scope_kind(options),
        diagnostic_error_count,
    }
}

fn resolve_artifact_scene_hint_options(
    app_id: &str,
    compile_options: &CompileOptions,
    access_path_scene: Option<&str>,
) -> Option<CompileOptions> {
    if compile_options
        .preview_target
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return None;
    }
    let scene_hint = compile_options
        .scene
        .as_deref()
        .or(access_path_scene)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let target_hint = host_api::access_scene_target_hint(app_id, scene_hint)?;
    Some(CompileOptions {
        scene: Some(scene_hint.to_string()),
        preview_target: Some(target_hint),
    })
}

fn preferred_build_options(
    app_id: &str,
    compile_options: &CompileOptions,
    access_path_scene: Option<&str>,
) -> (CompileOptions, &'static str) {
    if let Some(options) =
        resolve_artifact_scene_hint_options(app_id, compile_options, access_path_scene)
    {
        return (options, "scene_target_hint");
    }
    let reason = match compile_feedback_scope_kind(compile_options) {
        "scene_target" => "requested_scope",
        "scene_only" => "scene_only",
        "target_only" => "target_only",
        _ => "full_app",
    };
    (compile_options.clone(), reason)
}

fn resolved_artifact_feedback(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    path: &'static str,
) -> Option<ResolvedCompileOutcome> {
    let feedback = host_api::inspect_scoped_artifact(
        state,
        app_id,
        options.scene.clone(),
        options.preview_target.clone(),
    );
    let diagnostic_error_count = feedback.diagnostic_error_count;
    let reason = feedback.status.as_str();
    host_api::record_scoped_compile_feedback(
        app_id,
        options.scene.as_deref(),
        options.preview_target.as_deref(),
        &feedback,
    );
    let outcome = feedback.outcome?;
    Some(ResolvedCompileOutcome {
        outcome,
        feedback: compile_feedback(path, reason, options, diagnostic_error_count),
    })
}

fn resolved_build_feedback(
    app_id: &str,
    options: &CompileOptions,
    path: &'static str,
    reason: &'static str,
    outcome: CompileWithCacheOutcome,
) -> ResolvedCompileOutcome {
    let feedback = host_api::summarize_scoped_compile_feedback(outcome);
    let diagnostic_error_count = feedback.diagnostic_error_count;
    host_api::record_scoped_compile_feedback(
        app_id,
        options.scene.as_deref(),
        options.preview_target.as_deref(),
        &feedback,
    );
    let outcome = feedback
        .outcome
        .expect("scoped compile feedback must keep compile outcome");
    ResolvedCompileOutcome {
        outcome,
        feedback: compile_feedback(path, reason, options, diagnostic_error_count),
    }
}

pub(super) fn maybe_handle_compile_bootstrap_probe(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    compile_options: &CompileOptions,
    _components_root: &std::path::Path,
    access_path_scene: Option<&str>,
) -> Option<Response> {
    if !compile_bootstrap_route_supported(route_mode) {
        return None;
    }
    if !compile_bootstrap_probe_requested(query) {
        return None;
    }
    let feedback = host_api::inspect_scoped_artifact(
        state,
        app_id,
        compile_options.scene.clone(),
        compile_options.preview_target.clone(),
    );
    if feedback.status != host_api::ScopedFeedbackStatus::ArtifactMissing {
        return Some(compile_bootstrap_probe_response(
            true,
            feedback.status.as_str(),
        ));
    }
    let hinted = resolve_artifact_scene_hint_options(app_id, compile_options, access_path_scene)
        .and_then(|options| {
            host_api::inspect_scoped_artifact(
                state,
                app_id,
                options.scene.clone(),
                options.preview_target.clone(),
            )
            .status
            .artifact_ready()
            .then_some(options)
        });
    Some(compile_bootstrap_probe_response(
        hinted.is_some(),
        if hinted.is_some() {
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
    if let Some(outcome) =
        resolved_artifact_feedback(state, app_id, &compile_options, "artifact_only")
    {
        return CompileResolution::Outcome(outcome);
    }
    if let Some(hinted_options) =
        resolve_artifact_scene_hint_options(app_id, &compile_options, access_path_scene)
    {
        if let Some(outcome) =
            resolved_artifact_feedback(state, app_id, &hinted_options, "artifact_hint")
        {
            return CompileResolution::Outcome(outcome);
        }
    }
    if route_mode == UiRouteMode::Build {
        let (preferred_options, preferred_reason) =
            preferred_build_options(app_id, &compile_options, access_path_scene);
        let preferred_path = if compile_feedback_scope_kind(&preferred_options) == "full_app" {
            "full_build"
        } else {
            "scoped_build"
        };
        match compile_app_with_cache(state, app_id, &preferred_options, components_root.as_path()) {
            Ok(outcome) => {
                return CompileResolution::Outcome(resolved_build_feedback(
                    app_id,
                    &preferred_options,
                    preferred_path,
                    preferred_reason,
                    outcome,
                ));
            }
            Err(_) if !compile_options_equal(&preferred_options, &compile_options) => {
                match compile_app_with_cache(
                    state,
                    app_id,
                    &compile_options,
                    components_root.as_path(),
                ) {
                    Ok(outcome) => {
                        return CompileResolution::Outcome(resolved_build_feedback(
                            app_id,
                            &compile_options,
                            "build_fallback",
                            "preferred_scope_failed",
                            outcome,
                        ));
                    }
                    Err(_) => {
                        return CompileResolution::EarlyResponse(
                            Redirect::temporary(&build_source_fallback_location(app_id))
                                .into_response(),
                        );
                    }
                }
            }
            Err(_) => {
                return CompileResolution::EarlyResponse(
                    Redirect::temporary(&build_source_fallback_location(app_id)).into_response(),
                );
            }
        }
    }
    CompileResolution::EarlyResponse(render_access_artifact_unavailable(
        route_mode,
        app_id,
        compile_options.scene.as_deref().or(access_path_scene),
        manage_file,
    ))
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
    use super::{build_source_fallback_location, compile_feedback_scope_kind};
    use mei_lang_kernel::CompileOptions;

    #[test]
    fn build_source_fallback_uses_overview_tab() {
        assert_eq!(
            build_source_fallback_location("zhifa"),
            "/apps/build/zhifa?tab=overview"
        );
    }

    #[test]
    fn compile_feedback_scope_kind_distinguishes_requested_scope() {
        assert_eq!(
            compile_feedback_scope_kind(&CompileOptions {
                scene: Some("home".to_string()),
                preview_target: Some("main.mei".to_string()),
            }),
            "scene_target"
        );
        assert_eq!(
            compile_feedback_scope_kind(&CompileOptions {
                scene: Some("home".to_string()),
                preview_target: None,
            }),
            "scene_only"
        );
        assert_eq!(
            compile_feedback_scope_kind(&CompileOptions {
                scene: None,
                preview_target: Some("main.mei".to_string()),
            }),
            "target_only"
        );
        assert_eq!(
            compile_feedback_scope_kind(&CompileOptions::default()),
            "full_app"
        );
    }
}
