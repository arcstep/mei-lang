//! Build-route session layoutTuning draft merge (shared by full-page SSR and workspace-fragment).

use axum::http::HeaderMap;
use mei_lang_kernel::CompiledApp;

pub fn apply_build_session_layout_tuning_draft(
    compiled: &mut CompiledApp,
    workspace_root: &std::path::Path,
    app_id: &str,
    headers: &HeaderMap,
) -> bool {
    let session_id = mei_host_core::resolve_draft_session_id(headers);
    let storage_key =
        mei_host_core::layout_tuning_draft_storage_key(app_id, session_id.as_str());
    let draft = build_session_layout_tuning_draft(
        workspace_root,
        app_id,
        storage_key.as_str(),
    );
    if draft.is_none() {
        return false;
    }
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let config = mei_lang_kernel::load_mei_config_for_app(
        app_root.as_path(),
        Some(workspace_root),
    );
    let merged = mei_host_core::merge_layout_tuning_overlay(
        config.ops.layout_tuning.as_ref(),
        draft.as_ref(),
    );
    mei_host_graph::merge_layout_tuning_into_compiled(compiled, merged.as_ref());
    true
}

pub fn build_session_layout_tuning_draft(
    workspace_root: &std::path::Path,
    app_id: &str,
    storage_key: &str,
) -> Option<serde_json::Value> {
    if let Some(draft) = mei_host_core::layout_tuning_draft(storage_key) {
        return Some(draft);
    }
    crate::layout_tuning_draft_store::load_layout_tuning_draft_from_disk(
        workspace_root,
        app_id,
        storage_key,
    )
}
