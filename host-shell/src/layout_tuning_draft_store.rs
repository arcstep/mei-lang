//! Durable layoutTuning draft store (per app + session; survives process restart).

use std::fs;
use std::path::{Path, PathBuf};

use mei_host_core::set_layout_tuning_draft;
use mei_lang_kernel::resolve_app_root;
use serde_json::Value;

fn draft_store_dir(workspace_root: &Path, app_id: &str) -> PathBuf {
    let app_root = resolve_app_root(workspace_root, app_id);
    mei_lang_kernel::resolve_app_var_root(app_root.as_path()).join("layout-tuning-drafts")
}

fn draft_store_path(workspace_root: &Path, app_id: &str, storage_key: &str) -> PathBuf {
    let safe_key = storage_key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect::<String>();
    draft_store_dir(workspace_root, app_id).join(format!("{safe_key}.json"))
}

pub fn persist_layout_tuning_draft(
    workspace_root: &Path,
    app_id: &str,
    storage_key: &str,
    tuning: &Value,
) {
    set_layout_tuning_draft(storage_key, tuning.clone());
    let path = draft_store_path(workspace_root, app_id, storage_key);
    if tuning.is_null() {
        let _ = fs::remove_file(path.as_path());
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(tuning) {
        let _ = fs::write(path.as_path(), raw);
    }
}

pub fn load_layout_tuning_draft_from_disk(
    workspace_root: &Path,
    app_id: &str,
    storage_key: &str,
) -> Option<Value> {
    let path = draft_store_path(workspace_root, app_id, storage_key);
    let raw = fs::read_to_string(path.as_path()).ok()?;
    let value: Value = serde_json::from_str(raw.as_str()).ok()?;
    if !value.is_null() {
        set_layout_tuning_draft(storage_key, value.clone());
    }
    Some(value)
}

pub fn clear_layout_tuning_drafts_for_app(workspace_root: &Path, app_id: &str) -> usize {
    let dir = draft_store_dir(workspace_root, app_id);
    if !dir.is_dir() {
        return 0;
    }
    let count = fs::read_dir(dir.as_path())
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    let _ = fs::remove_dir_all(dir.as_path());
    count
}
