use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};

use crate::{opencode::bridge::BridgePromptRequest, AppState, SessionContextSnapshot};

use super::mei_scan::{build_mei_files_revision, collect_mei_file_entries};
use super::paths::{resolve_app_root, sanitize_relative_path};
use super::request_scope::world_scope_from_request;
use super::world_snapshot_lines::append_world_context_lines;

fn current_target_path(
    app_root: &FsPath,
    request: &BridgePromptRequest,
) -> Option<(String, PathBuf)> {
    let target = sanitize_relative_path(request.target_file.as_deref()?.trim())?;
    let path = app_root.join(&target);
    if !path.exists() || !path.is_file() {
        return None;
    }
    Some((target, path))
}

fn build_dynamic_mei_context(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let world_scope = world_scope_from_request(request);
    let scene_id = world_scope.scene_id.as_deref().unwrap_or("unknown");
    let entry_id = request
        .entry_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let mei_entries = collect_mei_file_entries(&state.source_root, &app_root);
    let mei_files = mei_entries
        .iter()
        .map(|item| item.relative_path.clone())
        .collect::<Vec<_>>();
    let target_context = current_target_path(&app_root, request).and_then(|(target, path)| {
        fs::read_to_string(&path)
            .ok()
            .map(|content| (target, content))
    });
    let mut lines = vec![
        "[MeiLang Runtime Context]".to_string(),
        format!("app: {app_id}"),
        format!("scene: {scene_id}"),
        format!("entry: {entry_id}"),
    ];
    if let Some((target, _)) = &target_context {
        lines.push(format!("target: {target}"));
    } else if let Some(target) = request.target_file.as_deref() {
        lines.push(format!("target: {}", target.trim()));
    }
    lines.push(
        "language: MeiLang .mei (Starlark-hosted DSL), not Music Encoding Initiative XML"
            .to_string(),
    );
    lines.push(String::new());
    lines.push("[Application Mei Files]".to_string());
    if mei_files.is_empty() {
        lines.push("- (none)".to_string());
    } else {
        lines.extend(mei_files.iter().map(|item| format!("- {item}")));
    }
    if let Some((target, content)) = target_context {
        lines.push(String::new());
        lines.push(format!("[Current File: {target}]"));
        lines.push("```mei".to_string());
        lines.push(content);
        lines.push("```".to_string());
    }
    append_world_context_lines(&mut lines, &state.source_root, &app_id, &world_scope);
    Some(lines.join("\n"))
}

pub(crate) fn build_dynamic_session_context_preview(
    state: &AppState,
    request: &BridgePromptRequest,
) -> Option<String> {
    build_dynamic_mei_context(state, request)
}

fn build_context_signature(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let scene_id = request.scene_id.as_deref().map(str::trim).unwrap_or("");
    let entry_id = request.entry_id.as_deref().map(str::trim).unwrap_or("");
    let target_file = request.target_file.as_deref().map(str::trim).unwrap_or("");
    let mei_entries = collect_mei_file_entries(&state.source_root, &app_root);
    let revision = build_mei_files_revision(&mei_entries);
    Some(format!(
        "v=world-context-v2|app={app_id}|scene={scene_id}|entry={entry_id}|target={target_file}|mei_revision={revision}"
    ))
}

pub(crate) fn load_or_refresh_session_context(
    state: &AppState,
    session_id: &str,
    request: &BridgePromptRequest,
) -> Option<String> {
    let signature = build_context_signature(state, request)?;
    {
        let Ok(cache) = state.opencode_session_context.lock() else {
            tracing::warn!("opencode session context cache lock poisoned; fallback to rebuild");
            return build_dynamic_mei_context(state, request);
        };
        if let Some(snapshot) = cache.get(session_id) {
            if snapshot.signature == signature {
                return Some(snapshot.context.clone());
            }
        }
    }
    let context = build_dynamic_mei_context(state, request)?;
    let Ok(mut cache) = state.opencode_session_context.lock() else {
        tracing::warn!("opencode session context cache lock poisoned; skip cache write");
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
        time::{SystemTime, UNIX_EPOCH},
    };

    use reqwest::Client as HttpClient;

    use super::*;
    use crate::opencode::ManagedOpencodeRuntime;

    fn build_test_state(source_root: PathBuf) -> AppState {
        AppState {
            package_root: Arc::new(source_root.clone()),
            source_root: Arc::new(source_root),
            opencode_preferred_mode: Arc::new("external".to_string()),
            opencode_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            opencode_auto_start: false,
            opencode_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            opencode_session_context: Arc::new(Mutex::new(HashMap::new())),
            opencode_http: Arc::new(HttpClient::new()),
        }
    }

    fn prepare_app_root() -> (PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mei_dynamic_context_test_{stamp}"));
        let app_root = root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(kind=\"app\", id=\"demo\", entries=[entry(id=\"main\", scene=\"s1\")])\n",
        )
        .expect("write main.mei");
        (root, app_root)
    }

    #[test]
    fn context_signature_tracks_scope_fields() {
        let (root, _) = prepare_app_root();
        let state = build_test_state(root.clone());
        let request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".to_string()),
            scene_id: Some("scene-a".to_string()),
            entry_id: Some("entry-a".to_string()),
            target_file: Some("main.mei".to_string()),
            system: None,
            agent: None,
            model: None,
        };
        let signature = build_context_signature(&state, &request).expect("signature");
        assert!(signature.contains("scene=scene-a"));
        assert!(signature.contains("entry=entry-a"));
        assert!(signature.contains("target=main.mei"));

        let mut changed = request.clone();
        changed.scene_id = Some("scene-b".to_string());
        let changed_signature = build_context_signature(&state, &changed).expect("changed signature");
        assert_ne!(signature, changed_signature);

        let _ = fs::remove_dir_all(&root);
    }
}
