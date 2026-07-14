mod browser;
mod host;
mod mei_body;

use crate::http::scene_api::WorldContextSnapshot;
use crate::{agent_runtime::bridge::BridgePromptRequest, AppState, SessionContextSnapshot};

use super::mei_scan::{build_mei_files_revision, collect_mei_file_entries};
use super::paths::resolve_app_root;
use super::scope_bundle::AgentScopeBundle;
use browser::browser_context_digest;
use host::host_protocol_digest;
use mei_body::{build_dynamic_mei_context, request_mode_slug};

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
    let browser = browser_context_digest(request);
    let host_protocol = host_protocol_digest(request);
    let host_contract_schema = request
        .host_contract_schema
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("na");
    let mei_entries = collect_mei_file_entries(&state.source_root, &app_root);
    let revision = build_mei_files_revision(&mei_entries);
    Some(format!(
        "v=world-context-v12|app={app_id}|scene={scene_id}|target={target_file}|mode={mode}|route={route}|rv={rv}|reach={reach}|browser={browser}|host_protocol={host_protocol}|host_contract_schema={host_contract_schema}|mei_revision={revision}"
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
            auth_enforcement: crate::auth::AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        }
    }

    fn prepare_app_root() -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("mei_dynamic_context_test_{}", Uuid::new_v4()));
        let app_root = root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(kind=\"app\", id=\"demo\", default_stage =\"s1\", scene=\"s1\")\nscene(id=\"s1\")\n",
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
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let signature = build_context_signature(&state, &request).expect("signature");
        assert!(signature.contains("scene=scene-a"));
        assert!(signature.contains("target=main.mei"));
        assert!(signature.contains("v=world-context-v12"));

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
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let ctx =
            mei_body::build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
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
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let ctx =
            mei_body::build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
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
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let ctx =
            mei_body::build_dynamic_mei_context(&state, &request, None, None).unwrap_or_default();
        assert!(ctx.contains("[Build mode — current target .mei snapshot]"));
        assert!(ctx.contains("app(kind=\"app\""));
        let _ = fs::remove_dir_all(&root);
    }
}
