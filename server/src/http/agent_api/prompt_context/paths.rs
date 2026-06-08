use std::path::{Component, Path as FsPath, PathBuf};

use mei_lang_kernel::resolve_app_root as resolve_workspace_app_root;

use crate::{agent_runtime::bridge::BridgePromptRequest, AppState};

pub(crate) fn sanitize_relative_path(value: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in FsPath::new(value).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub(crate) fn resolve_app_root(
    state: &AppState,
    request: &BridgePromptRequest,
) -> Option<(String, PathBuf)> {
    let app_id = request.app_id.as_deref()?.trim();
    if app_id.is_empty() {
        return None;
    }
    let root = resolve_workspace_app_root(state.source_root.as_path(), app_id);
    if !root.exists() {
        return None;
    }
    Some((app_id.to_string(), root))
}
