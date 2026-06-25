use std::path::PathBuf;
use std::time::Instant;

use axum::{
    http::StatusCode,
    http::{HeaderName, HeaderValue},
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::{HostAccountView, TopbarMenuContext, UiRouteMode, UploadFileEntry};
use mei_lang_kernel::{CompileOptions, WorkspaceAppMeta};

use crate::http::compile_cache::{
    build_preview_diagnostic_error_count, resolve_build_preview_compile,
};
use crate::http::host_api;
use crate::http::host_error_page::{self, HostScopedRebuildContext, HostShellAction};
use crate::AppState;
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
    pub outcome: crate::http::compile_cache::CompileWithCacheOutcome,
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
    if feedback.diagnostic_error_count > 0 {
        host_api::record_scoped_compile_feedback(
            app_id,
            options.scene.as_deref(),
            options.preview_target.as_deref(),
            &feedback,
        );
        return None;
    }
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
    query: &AppQuery,
    compile_options: CompileOptions,
    _components_root: PathBuf,
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
    let gate = crate::readiness::scope_gate::resolve_scope_gate_for_compile(
        state.source_root.as_path(),
        app_id,
        route_mode,
        &compile_options,
        query,
    );
    if route_mode == UiRouteMode::Build
        && compile_options
            .preview_target
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    {
        match resolve_build_preview_compile(
            state,
            app_id,
            &compile_options,
            _components_root.as_path(),
        ) {
            Ok(Some(outcome)) => {
                let diagnostic_error_count =
                    build_preview_diagnostic_error_count(&outcome.compiled);
                let reason = if diagnostic_error_count > 0 {
                    "degraded"
                } else if outcome.cache_hit {
                    "ready"
                } else {
                    "compiled"
                };
                return CompileResolution::Outcome(ResolvedCompileOutcome {
                    outcome,
                    feedback: compile_feedback(
                        "build_preview_compile",
                        reason,
                        &compile_options,
                        diagnostic_error_count,
                    ),
                });
            }
            Ok(None) => {}
            Err(failure) => {
                tracing::warn!(
                    app_id = %app_id,
                    preview_target = %compile_options.preview_target.as_deref().unwrap_or("-"),
                    error = %failure.error,
                    "build preview scoped compile failed; falling back to artifact-only path"
                );
            }
        }
    }
    if !gate.shell_ready {
        let scene_hint = gate.resolved_scene.clone();
        let target_hint = gate.resolved_target.clone();
        return CompileResolution::EarlyResponse(render_artifact_unavailable(
            state.source_root.as_path(),
            route_mode,
            app_id,
            scene_hint.as_deref(),
            target_hint.as_deref().or(manage_file),
            gate,
        ));
    }
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
    let scene_hint = gate.resolved_scene.clone();
    let target_hint = gate.resolved_target.clone();
    CompileResolution::EarlyResponse(render_artifact_unavailable(
        state.source_root.as_path(),
        route_mode,
        app_id,
        scene_hint.as_deref(),
        target_hint.as_deref().or(manage_file),
        gate,
    ))
}

fn render_artifact_unavailable(
    _source_root: &std::path::Path,
    route_mode: UiRouteMode,
    app_id: &str,
    scene_hint: Option<&str>,
    target_hint: Option<&str>,
    scope_gate: crate::readiness::scope_gate::ScopeGateReport,
) -> Response {
    let is_build = route_mode == UiRouteMode::Build;
    let mut actions = vec![HostShellAction {
        href: "/".to_string(),
        label: "返回首页".to_string(),
        primary: !is_build,
    }];
    if is_build {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/build/{app_id}?tab=overview"),
                label: "打开构建概览".to_string(),
                primary: false,
            },
        );
    } else if let Some(scene_id) = scene_hint.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/app/{app_id}/scene/{scene_id}?chrome=none"),
                label: "重试当前场景".to_string(),
                primary: true,
            },
        );
    } else if let Some(target) = target_hint.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/build/{app_id}?file={target}"),
                label: "打开构建视图".to_string(),
                primary: false,
            },
        );
    }
    let gate = host_api::artifact_gate_status(app_id, scene_hint, target_hint);
    let blocker_detail = if scope_gate.blockers.is_empty() {
        gate.last_error.clone()
    } else {
        Some(scope_gate.blockers.join("; "))
    };
    let message = if is_build {
        blocker_detail.as_deref().map_or_else(
            || {
                "当前 scope 产物尚未 materialize。宿主读路径不会触发 Starlark 编译；请使用「重建此 scope」或等待 prebuild 完成。"
                    .to_string()
            },
            |error| {
                format!(
                    "当前构建预览命中未就绪产物；请先 scoped rebuild。最近一次构建状态：{error}"
                )
            },
        )
    } else {
        blocker_detail.as_deref().map_or_else(
            || {
                "当前 access-only 宿主已切到 artifact-first 主路径，请先等待后台构建完成，或在构建视图中手动触发重建。"
                    .to_string()
            },
            |error| {
                format!(
                    "当前访问请求命中了未就绪产物；宿主不会因访问触发编译。最近一次构建状态：{error}"
                )
            },
        )
    };
    let detail = match (
        scene_hint.map(str::trim).filter(|value| !value.is_empty()),
        target_hint.map(str::trim).filter(|value| !value.is_empty()),
    ) {
        (Some(scene_id), Some(target)) => Some(format!(
            "mode={} app={app_id} scene={scene_id} target={target} host_phase={} app_phase={} scope_phase={} blockers={}",
            route_mode.slug(),
            gate.host_phase,
            gate.app_phase.as_deref().unwrap_or("missing"),
            gate.scope_phase.as_deref().unwrap_or("missing"),
            scope_gate.blockers.join("|"),
        )),
        (Some(scene_id), None) => Some(format!(
            "mode={} app={app_id} scene={scene_id} host_phase={} app_phase={} scope_phase={}",
            route_mode.slug(),
            gate.host_phase,
            gate.app_phase.as_deref().unwrap_or("missing"),
            gate.scope_phase.as_deref().unwrap_or("missing"),
        )),
        _ => Some(format!(
            "mode={} app={app_id} host_phase={} app_phase={} scope_phase={}",
            route_mode.slug(),
            gate.host_phase,
            gate.app_phase.as_deref().unwrap_or("missing"),
            gate.scope_phase.as_deref().unwrap_or("missing"),
        )),
    };
    let headline = if is_build {
        "构建 scope 产物尚未就绪"
    } else {
        "访问态产物尚未就绪"
    };
    let rebuild_block = if is_build {
        host_error_page::render_scoped_rebuild_block(&HostScopedRebuildContext {
            app_id: app_id.to_string(),
            scene_id: scene_hint
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            target_file: target_hint
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
    } else {
        String::new()
    };
    let mut html = host_error_page::render_error_page(
        StatusCode::SERVICE_UNAVAILABLE,
        headline,
        message.as_str(),
        detail.as_deref(),
        &actions,
    );
    if !rebuild_block.is_empty() {
        html = html.replace("</main>", &format!("{rebuild_block}</main>"));
        html.push_str(&host_error_page::render_scoped_rebuild_script());
    }
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
    use super::compile_feedback_scope_kind;
    use mei_lang_kernel::CompileOptions;

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
