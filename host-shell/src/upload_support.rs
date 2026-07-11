use std::path::{Path, PathBuf};

use mei_lang_app::UploadFileEntry;
use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root};

use crate::api_error::ApiError;
use crate::state::SharedState;

pub(crate) fn workspace_root_from_state(state: &SharedState) -> PathBuf {
    state.read().expect("state lock").ctx.workspace_root.clone()
}

pub(crate) fn resolve_upload_root(state: &SharedState, app_id: &str) -> Result<PathBuf, ApiError> {
    let workspace_root = workspace_root_from_state(state);
    let app_root = resolve_app_root(workspace_root.as_path(), app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root.as_path()));
    let rel = config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::status(
                axum::http::StatusCode::NOT_FOUND,
                "app has no paths.upload configured",
            )
        })?;
    Ok(app_root.join(rel))
}

pub(crate) fn upload_rel_from_config(app_root: &Path, workspace_root: &Path) -> Option<String> {
    let config = load_mei_config_for_app(app_root, Some(workspace_root));
    config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
}

pub(crate) fn list_upload_files(upload_root: &Path, _upload_rel: &str) -> Vec<UploadFileEntry> {
    fn walk(dir: &Path, rel_prefix: &str, out: &mut Vec<UploadFileEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = if rel_prefix.is_empty() {
                name.clone()
            } else {
                format!("{rel_prefix}/{name}")
            };
            out.push(UploadFileEntry {
                path: path.clone(),
                name,
                is_dir: file_type.is_dir(),
                size_bytes: entry.metadata().ok().and_then(|meta| {
                    if file_type.is_dir() {
                        None
                    } else {
                        Some(meta.len())
                    }
                }),
                modified_ms: entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis() as u64),
                modified_label: None,
            });
            if file_type.is_dir() {
                walk(&entry.path(), path.as_str(), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(upload_root, "", &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub(crate) fn invalidate_after_upload(state: &SharedState, app_id: &str) {
    let _ = app_id;
    if let Ok(mut guard) = state.write() {
        crate::build_ops::refresh_materialization_flags(&mut guard);
    }
}

pub(crate) fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("csv") => "text/csv",
        _ => "application/octet-stream",
    }
}
