use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use axum::http::StatusCode;
use mei_lang_kernel::load_mei_config_for_app;

use crate::{AppError, AppState};

use super::types::{self, *};

pub(super) fn resolve_upload_root(state: &AppState, app_id: &str) -> Result<PathBuf, AppError> {
    let app_root = state.source_root.join(app_id.trim_start_matches('/'));
    let config = load_mei_config_for_app(&app_root, Some(state.source_root.as_path()));
    let rel = config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::status(StatusCode::NOT_FOUND, "app has no paths.upload configured")
        })?;
    Ok(app_root.join(rel))
}

pub(super) fn sanitize_upload_rel(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "empty upload path",
        ));
    }
    let path = Path::new(&trimmed);
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::status(
                    StatusCode::BAD_REQUEST,
                    "upload path must stay within upload directory",
                ));
            }
            Component::CurDir => {}
            Component::Normal(_) => {}
        }
    }
    Ok(trimmed.trim_start_matches('/').to_string())
}

pub(super) fn sanitize_upload_id(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 80
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "invalid upload session id",
        ));
    }
    Ok(trimmed.to_string())
}

pub(super) fn canonical_upload_root(upload_root: &Path) -> Result<PathBuf, AppError> {
    upload_root
        .canonicalize()
        .map_err(|error| AppError::msg(format!("upload root unavailable: {error}")))
}

pub(super) fn resolve_upload_target(upload_root: &Path, rel: &str) -> Result<PathBuf, AppError> {
    let rel = sanitize_upload_rel(rel)?;
    let canonical_root = canonical_upload_root(upload_root)?;
    let resolved = canonical_root.join(&rel);
    if !resolved.starts_with(&canonical_root) {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "upload path escapes upload directory",
        ));
    }
    Ok(resolved)
}

pub(super) fn resolve_existing_upload_file(
    upload_root: &Path,
    rel: &str,
) -> Result<PathBuf, AppError> {
    let rel = sanitize_upload_rel(rel)?;
    let canonical_root = canonical_upload_root(upload_root)?;
    let canonical_file = canonical_root
        .join(&rel)
        .canonicalize()
        .map_err(|error| AppError::msg(format!("upload file not found: {error}")))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "upload path escapes upload directory",
        ));
    }
    Ok(canonical_file)
}

pub(super) fn build_upload_rel(
    upload_dir: Option<&str>,
    file_name: &str,
) -> Result<String, AppError> {
    let clean_file_name = sanitize_upload_rel(file_name)?;
    if let Some(dir) = upload_dir {
        let clean_dir = sanitize_upload_rel(dir)?;
        Ok(format!("{clean_dir}/{clean_file_name}"))
    } else {
        Ok(clean_file_name)
    }
}

pub(super) fn file_name_from_upload_rel(rel: &str) -> Result<String, AppError> {
    let rel = sanitize_upload_rel(rel)?;
    Path::new(&rel)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "invalid upload file name"))
}

pub(super) fn build_move_target_rel(
    from_rel: &str,
    to_dir: Option<&str>,
) -> Result<String, AppError> {
    let file_name = file_name_from_upload_rel(from_rel)?;
    let target_dir = to_dir.map(str::trim).filter(|value| !value.is_empty());
    build_upload_rel(target_dir, &file_name)
}

pub(super) fn build_rename_target_rel(from_rel: &str, to_path: &str) -> Result<String, AppError> {
    let from_rel = sanitize_upload_rel(from_rel)?;
    let to_rel = sanitize_upload_rel(to_path)?;
    if to_rel == from_rel {
        return Ok(from_rel);
    }
    Ok(to_rel)
}

pub(super) fn upload_chunk_sessions_root(upload_root: &Path) -> PathBuf {
    upload_root.join(".mei-upload-sessions")
}

pub(super) fn upload_chunk_session_dir(
    upload_root: &Path,
    upload_id: &str,
) -> Result<PathBuf, AppError> {
    let upload_id = sanitize_upload_id(upload_id)?;
    Ok(upload_chunk_sessions_root(upload_root).join(upload_id))
}

pub(super) fn upload_chunk_meta_path(session_dir: &Path) -> PathBuf {
    session_dir.join("session.json")
}

pub(super) fn upload_chunk_part_path(session_dir: &Path, index: usize) -> PathBuf {
    session_dir.join(format!("{index:06}.part"))
}

pub(super) fn normalize_chunk_size(chunk_size: usize) -> usize {
    chunk_size.clamp(types::MIN_UPLOAD_CHUNK_BYTES, types::MAX_UPLOAD_CHUNK_BYTES)
}

pub(super) fn total_chunk_count(size_bytes: u64, chunk_size: usize) -> Result<usize, AppError> {
    if size_bytes == 0 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "empty upload file",
        ));
    }
    let chunk_size_u64 = chunk_size as u64;
    Ok(size_bytes.div_ceil(chunk_size_u64) as usize)
}

pub(super) fn stable_upload_id(
    rel_path: &str,
    size_bytes: u64,
    chunk_size: usize,
    last_modified_ms: Option<u64>,
) -> String {
    let fingerprint = format!(
        "{rel_path}\n{size_bytes}\n{chunk_size}\n{}",
        last_modified_ms.unwrap_or_default()
    );
    let mut hash = 0xcbf29ce484222325u64;
    for byte in fingerprint.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("u{hash:016x}")
}

pub(super) fn write_chunk_session_meta(
    session_dir: &Path,
    meta: &UploadChunkSessionMeta,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|error| AppError::msg(format!("encode upload session failed: {error}")))?;
    fs::write(upload_chunk_meta_path(session_dir), bytes)
        .map_err(|error| AppError::msg(format!("write upload session failed: {error}")))
}

pub(super) fn read_chunk_session_meta(
    session_dir: &Path,
) -> Result<UploadChunkSessionMeta, AppError> {
    let bytes = fs::read(upload_chunk_meta_path(session_dir))
        .map_err(|error| AppError::msg(format!("read upload session failed: {error}")))?;
    serde_json::from_slice::<UploadChunkSessionMeta>(&bytes)
        .map_err(|error| AppError::msg(format!("decode upload session failed: {error}")))
}

pub(super) fn list_uploaded_chunk_indexes(session_dir: &Path) -> Result<Vec<usize>, AppError> {
    let mut out = Vec::new();
    if !session_dir.exists() {
        return Ok(out);
    }
    let entries = fs::read_dir(session_dir)
        .map_err(|error| AppError::msg(format!("read upload session dir failed: {error}")))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".part") {
            continue;
        }
        let Some(stem) = name.strip_suffix(".part") else {
            continue;
        };
        if let Ok(index) = stem.parse::<usize>() {
            out.push(index);
        }
    }
    out.sort_unstable();
    Ok(out)
}

pub(super) fn expected_chunk_len(meta: &UploadChunkSessionMeta, index: usize) -> usize {
    let chunk_size = meta.chunk_size as u64;
    let start = index as u64 * chunk_size;
    let end = std::cmp::min(start + chunk_size, meta.size_bytes);
    end.saturating_sub(start) as usize
}
