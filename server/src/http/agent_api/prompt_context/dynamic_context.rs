use std::fs;

use crate::http::scene_api::WorldContextSnapshot;
use crate::{agent_runtime::bridge::BridgePromptRequest, AppState, SessionContextSnapshot};

use super::mei_scan::{build_mei_files_revision, collect_mei_file_entries};
use super::paths::{resolve_app_root, sanitize_relative_path};
use super::request_scope::world_scope_from_request;
use super::scope_bundle::AgentScopeBundle;
use super::world_snapshot_lines::{
    append_world_context_error_lines, append_world_context_lines,
    append_world_context_snapshot_lines,
};

const ASK_INLINE_TARGET_MAX_BYTES: usize = 24 * 1024;

fn request_mode_slug(request: &BridgePromptRequest) -> &'static str {
    let mode = request
        .mode
        .as_deref()
        .or(request.agent.as_deref())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "build".to_string());
    if mode == "ask" || mode == "plan" {
        "ask"
    } else {
        "build"
    }
}

fn resolve_target_path_for_request(
    state: &AppState,
    app_id: &str,
    request: &BridgePromptRequest,
) -> Option<(String, std::path::PathBuf)> {
    let raw_target = request
        .target_file
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())?;
    let target_rel = sanitize_relative_path(raw_target)?;
    let mut candidates = vec![(target_rel.clone(), state.source_root.join(&target_rel))];
    let app_prefixed = format!("{app_id}/{target_rel}");
    if app_prefixed != target_rel {
        candidates.push((app_prefixed.clone(), state.source_root.join(&app_prefixed)));
    }
    candidates
        .into_iter()
        .find(|(_, full)| full.exists() && full.is_file())
}

fn build_dynamic_mei_context(
    state: &AppState,
    request: &BridgePromptRequest,
    world_snapshot: Option<&WorldContextSnapshot>,
    world_snapshot_error: Option<&str>,
) -> Option<String> {
    let (app_id, _app_root) = resolve_app_root(state, request)?;
    let ask_mode = request_mode_slug(request) == "ask";
    let world_scope = world_scope_from_request(request);
    let scene_id = world_scope.scene_id.as_deref().unwrap_or("unknown");
    let mut lines = vec![
        "[MeiLang Runtime Context]".to_string(),
        format!("app: {app_id}"),
        format!("scene: {scene_id}"),
    ];
    if let Some(target) = request
        .target_file
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        if sanitize_relative_path(target).is_some() {
            lines.push(format!("target: {target}"));
        } else {
            lines.push(format!("target: {target} (invalid relative path)"));
        }
    }
    if let Some(snapshot) = world_snapshot {
        append_world_context_snapshot_lines(&mut lines, snapshot);
    } else if let Some(message) = world_snapshot_error {
        append_world_context_error_lines(&mut lines, &app_id, message);
    } else {
        append_world_context_lines(&mut lines, &state.source_root, &app_id, &world_scope);
    }
    lines.push(String::new());
    if ask_mode {
        lines.push(
            concat!(
                "[Ask mode — world-first]\n",
                "The active target `.mei` source is not inlined here so the prompt stays focused on the injected world/runtime catalog.\n",
                "Use `dataset_query` / `dataset_metric` for tabular data and metrics.\n",
                "Use `read_file` only when you need verbatim DSL from a workspace path that is allowed by the current resource visibility (typically under `<app_id>/...`).",
            )
            .to_string(),
        );
    } else {
        lines.push(format!(
            "[Build mode — scene anchor] scene_id={scene_id} (source-focus file body inlined below when available)"
        ));
        lines.push(String::new());
        if let Some((target_rel, full_path)) =
            resolve_target_path_for_request(state, &app_id, request)
        {
            match fs::read_to_string(&full_path) {
                Ok(content) => {
                    let bytes = content.as_bytes();
                    let (inlined, truncated) = if bytes.len() > ASK_INLINE_TARGET_MAX_BYTES {
                        (
                            String::from_utf8_lossy(&bytes[..ASK_INLINE_TARGET_MAX_BYTES])
                                .to_string(),
                            true,
                        )
                    } else {
                        (content, false)
                    };
                    lines.push("[Build mode — current target .mei snapshot]".to_string());
                    lines.push(format!("path: {target_rel}"));
                    lines.push(format!(
                        "truncated: {} (max {} bytes)",
                        if truncated { "yes" } else { "no" },
                        ASK_INLINE_TARGET_MAX_BYTES
                    ));
                    lines.push("---".to_string());
                    lines.push(inlined);
                }
                Err(error) => {
                    lines.push(format!(
                    "[Build mode — current target .mei snapshot]\npath: {target_rel}\nerror: failed to read target file ({error})"
                ));
                }
            }
            lines.push(String::new());
            lines.push(
            "Other scene/world/frame files are indexed in the injected world/runtime catalog above, not inlined; use `read_file` within allowed paths for source-focus edits."
                .to_string(),
        );
        } else {
            lines.push(
                "[Build mode — current target .mei snapshot]\nunavailable: no valid target `.mei` in current request scope"
                    .to_string(),
            );
            lines.push(String::new());
            lines.push(
                concat!(
                    "`.mei` source is not inlined above. `read_file` paths are relative to the workspace root (parent of each app folder). ",
                    "For app-owned files use `<app_id>/...` (e.g. `spbjw/scenes/行政检查/datasets/...`); a bare `scenes/...` or `data/...` without app id resolves next to the workspace root and is usually wrong."
                )
                .to_string(),
            );
        }
    }
    Some(lines.join("\n"))
}

pub(crate) fn build_dynamic_session_context_preview(
    state: &AppState,
    request: &BridgePromptRequest,
    world_snapshot: Option<&WorldContextSnapshot>,
    world_snapshot_error: Option<&str>,
) -> Option<String> {
    build_dynamic_mei_context(state, request, world_snapshot, world_snapshot_error)
}

fn build_context_signature(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let scene_id = request.scene_id.as_deref().map(str::trim).unwrap_or("");
    let target_file = request.target_file.as_deref().map(str::trim).unwrap_or("");
    let mode = request_mode_slug(request);
    let route = request.route_mode.as_deref().map(str::trim).unwrap_or("");
    let bundle = AgentScopeBundle::resolve(state, request)?;
    let rv = bundle.profile.resource_visibility.as_slug();
    let reach = bundle.reach_digest.clone();
    let mei_entries = collect_mei_file_entries(&state.source_root, &app_root);
    let revision = build_mei_files_revision(&mei_entries);
    Some(format!(
        "v=world-context-v8|app={app_id}|scene={scene_id}|target={target_file}|mode={mode}|route={route}|rv={rv}|reach={reach}|mei_revision={revision}"
    ))
}

pub(crate) fn load_or_refresh_session_context(
    state: &AppState,
    session_id: &str,
    request: &BridgePromptRequest,
) -> Option<String> {
    let signature = build_context_signature(state, request)?;
    {
        let Ok(cache) = state.agent_session_context.lock() else {
            tracing::warn!("agent session context cache lock poisoned; fallback to rebuild");
            let b = AgentScopeBundle::resolve(state, request)?;
            return build_dynamic_mei_context(
                state,
                request,
                b.snapshot.as_ref(),
                b.snapshot_error.as_deref(),
            );
        };
        if let Some(snapshot) = cache.get(session_id) {
            if snapshot.signature == signature {
                return Some(snapshot.context.clone());
            }
        }
    }
    let b = AgentScopeBundle::resolve(state, request)?;
    let context = build_dynamic_mei_context(
        state,
        request,
        b.snapshot.as_ref(),
        b.snapshot_error.as_deref(),
    )?;
    let Ok(mut cache) = state.agent_session_context.lock() else {
        tracing::warn!("agent session context cache lock poisoned; skip cache write");
        return Some(context);
    };
    cache.insert(
        session_id.to_string(),
        SessionContextSnapshot {
            signature,
            context: context.clone(),
        },
    );
    Some(context)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use uuid::Uuid;

    use super::*;
    use crate::agent_runtime::ManagedOpencodeRuntime;

    fn build_test_state(package_root: PathBuf, source_root: PathBuf) -> AppState {
        let source_root = Arc::new(source_root);
        let native_agent = Arc::new(
            crate::mei_agent::NativeAgent::open_with_resource_tools(
                source_root.as_ref().clone(),
                package_root.clone(),
                Arc::new(crate::mei_agent::resource_tools::NoopResourceToolExecutor::default()),
            )
            .expect("native"),
        );
        AppState {
            package_root: Arc::new(package_root),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
            gis_tiles: Arc::new(crate::gis_config::GisTilesConfig::resolve()),
        }
    }

    fn prepare_app_root() -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("mei_dynamic_context_test_{}", Uuid::new_v4()));
        let app_root = root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(kind=\"app\", id=\"demo\", default_scene=\"s1\", scene=\"s1\")\nscene(id=\"s1\")\n",
        )
        .expect("write main.mei");
        (root, app_root)
    }

    #[test]
    fn context_signature_tracks_scope_fields() {
        let (root, _app_root) = prepare_app_root();
        let state = build_test_state(root.clone(), root.clone());
        let request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".to_string()),
            scene_id: Some("scene-a".to_string()),
            target_file: Some("main.mei".to_string()),
            system: None,
            mode: None,
            route_mode: None,
            agent: None,
            model: None,
            resource_visibility: None,
        };
        let signature = build_context_signature(&state, &request).expect("signature");
        assert!(signature.contains("scene=scene-a"));
        assert!(signature.contains("target=main.mei"));
        assert!(signature.contains("v=world-context-v8"));

        let mut changed = request.clone();
        changed.resource_visibility = Some("local_only".into());
        let changed_signature =
            build_context_signature(&state, &changed).expect("changed signature");
        assert_ne!(signature, changed_signature);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dynamic_context_does_not_embed_mei_fence() {
        let (root, _app_root) = prepare_app_root();
        let state = build_test_state(root.clone(), root.clone());
        let request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".to_string()),
            scene_id: Some("s1".to_string()),
            target_file: Some("main.mei".to_string()),
            system: None,
            mode: None,
            route_mode: None,
            agent: None,
            model: None,
            resource_visibility: None,
        };
        let ctx = build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
        assert!(
            !ctx.contains("```mei"),
            "expected no inlined mei fence: {ctx}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ask_mode_world_first_skips_target_mei_snapshot() {
        let (root, _app_root) = prepare_app_root();
        let state = build_test_state(root.clone(), root.clone());
        let request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".to_string()),
            scene_id: Some("s1".to_string()),
            target_file: Some("main.mei".to_string()),
            system: None,
            mode: Some("ask".to_string()),
            route_mode: Some("access".to_string()),
            agent: None,
            model: None,
            resource_visibility: None,
        };
        let ctx = build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
        assert!(ctx.contains("[Ask mode — world-first]"));
        assert!(!ctx.contains("[Build mode — current target .mei snapshot]"));
        assert!(!ctx.contains("[Build mode"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn build_mode_inlines_current_target_snapshot() {
        let (root, _app_root) = prepare_app_root();
        let state = build_test_state(root.clone(), root.clone());
        let request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".to_string()),
            scene_id: Some("s1".to_string()),
            target_file: Some("main.mei".to_string()),
            system: None,
            mode: Some("build".to_string()),
            route_mode: Some("manage".to_string()),
            agent: None,
            model: None,
            resource_visibility: None,
        };
        let ctx = build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
        assert!(ctx.contains("[Build mode — current target .mei snapshot]"));
        assert!(ctx.contains("app(kind=\"app\""));
        let _ = fs::remove_dir_all(&root);
    }
}
