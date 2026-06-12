use std::{
    fs,
    io::{BufWriter, Write},
    path::{Component, Path, PathBuf},
};

use axum::{
    body::Bytes,
    extract::{Json as AxumJson, Multipart, Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::load_mei_config_for_app;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{AppError, AppState};

const MIN_UPLOAD_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_UPLOAD_CHUNK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct UploadDeleteQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkStatusQuery {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkPutQuery {
    pub upload_id: String,
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkInitRequest {
    pub file_name: String,
    pub dir: Option<String>,
    pub size_bytes: u64,
    pub chunk_size: usize,
    pub last_modified_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkCompleteRequest {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadMoveRequest {
    pub from_path: String,
    pub to_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadDirCreateRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadRenameRequest {
    pub from_path: String,
    pub to_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UploadChunkSessionMeta {
    upload_id: String,
    rel_path: String,
    file_name: String,
    size_bytes: u64,
    chunk_size: usize,
    total_chunks: usize,
    last_modified_ms: Option<u64>,
}

fn resolve_upload_root(state: &AppState, app_id: &str) -> Result<PathBuf, AppError> {
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

fn sanitize_upload_rel(raw: &str) -> Result<String, AppError> {
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

fn sanitize_upload_id(raw: &str) -> Result<String, AppError> {
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

fn canonical_upload_root(upload_root: &Path) -> Result<PathBuf, AppError> {
    upload_root
        .canonicalize()
        .map_err(|error| AppError::msg(format!("upload root unavailable: {error}")))
}

fn resolve_upload_target(upload_root: &Path, rel: &str) -> Result<PathBuf, AppError> {
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

fn resolve_existing_upload_file(upload_root: &Path, rel: &str) -> Result<PathBuf, AppError> {
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

fn build_upload_rel(upload_dir: Option<&str>, file_name: &str) -> Result<String, AppError> {
    let clean_file_name = sanitize_upload_rel(file_name)?;
    if let Some(dir) = upload_dir {
        let clean_dir = sanitize_upload_rel(dir)?;
        Ok(format!("{clean_dir}/{clean_file_name}"))
    } else {
        Ok(clean_file_name)
    }
}

fn file_name_from_upload_rel(rel: &str) -> Result<String, AppError> {
    let rel = sanitize_upload_rel(rel)?;
    Path::new(&rel)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "invalid upload file name"))
}

fn build_move_target_rel(from_rel: &str, to_dir: Option<&str>) -> Result<String, AppError> {
    let file_name = file_name_from_upload_rel(from_rel)?;
    let target_dir = to_dir.map(str::trim).filter(|value| !value.is_empty());
    build_upload_rel(target_dir, &file_name)
}

fn build_rename_target_rel(from_rel: &str, to_path: &str) -> Result<String, AppError> {
    let from_rel = sanitize_upload_rel(from_rel)?;
    let to_rel = sanitize_upload_rel(to_path)?;
    if to_rel == from_rel {
        return Ok(from_rel);
    }
    Ok(to_rel)
}

fn upload_chunk_sessions_root(upload_root: &Path) -> PathBuf {
    upload_root.join(".mei-upload-sessions")
}

fn upload_chunk_session_dir(upload_root: &Path, upload_id: &str) -> Result<PathBuf, AppError> {
    let upload_id = sanitize_upload_id(upload_id)?;
    Ok(upload_chunk_sessions_root(upload_root).join(upload_id))
}

fn upload_chunk_meta_path(session_dir: &Path) -> PathBuf {
    session_dir.join("session.json")
}

fn upload_chunk_part_path(session_dir: &Path, index: usize) -> PathBuf {
    session_dir.join(format!("{index:06}.part"))
}

fn normalize_chunk_size(chunk_size: usize) -> usize {
    chunk_size.clamp(MIN_UPLOAD_CHUNK_BYTES, MAX_UPLOAD_CHUNK_BYTES)
}

fn total_chunk_count(size_bytes: u64, chunk_size: usize) -> Result<usize, AppError> {
    if size_bytes == 0 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "empty upload file",
        ));
    }
    let chunk_size_u64 = chunk_size as u64;
    Ok(size_bytes.div_ceil(chunk_size_u64) as usize)
}

fn stable_upload_id(
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

fn write_chunk_session_meta(
    session_dir: &Path,
    meta: &UploadChunkSessionMeta,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|error| AppError::msg(format!("encode upload session failed: {error}")))?;
    fs::write(upload_chunk_meta_path(session_dir), bytes)
        .map_err(|error| AppError::msg(format!("write upload session failed: {error}")))
}

fn read_chunk_session_meta(session_dir: &Path) -> Result<UploadChunkSessionMeta, AppError> {
    let bytes = fs::read(upload_chunk_meta_path(session_dir))
        .map_err(|error| AppError::msg(format!("read upload session failed: {error}")))?;
    serde_json::from_slice::<UploadChunkSessionMeta>(&bytes)
        .map_err(|error| AppError::msg(format!("decode upload session failed: {error}")))
}

fn list_uploaded_chunk_indexes(session_dir: &Path) -> Result<Vec<usize>, AppError> {
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

fn expected_chunk_len(meta: &UploadChunkSessionMeta, index: usize) -> usize {
    let chunk_size = meta.chunk_size as u64;
    let start = index as u64 * chunk_size;
    let end = std::cmp::min(start + chunk_size, meta.size_bytes);
    end.saturating_sub(start) as usize
}

pub async fn upload_file_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    std::fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;
    let mut upload_dir: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::msg(format!("multipart read failed: {error}")))?
    {
        match field.name() {
            Some("dir") => {
                upload_dir =
                    Some(field.text().await.map_err(|error| {
                        AppError::msg(format!("read upload dir failed: {error}"))
                    })?);
            }
            Some("file") => {
                file_name = field.file_name().map(str::to_string);
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| {
                            AppError::msg(format!("read upload bytes failed: {error}"))
                        })?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }
    let file_name = file_name
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "missing upload file"))?;
    let file_bytes = file_bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "empty upload file"))?;
    let rel = build_upload_rel(upload_dir.as_deref(), &file_name)?;
    let target = resolve_upload_target(&upload_root, &rel)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AppError::msg(format!("failed to create upload parent: {error}")))?;
    }
    fs::write(&target, file_bytes)
        .map_err(|error| AppError::msg(format!("failed to write upload file: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": rel })))
}

pub async fn upload_chunk_init_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadChunkInitRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;

    let chunk_size = normalize_chunk_size(request.chunk_size);
    let total_chunks = total_chunk_count(request.size_bytes, chunk_size)?;
    let rel_path = build_upload_rel(request.dir.as_deref(), &request.file_name)?;
    let upload_id = stable_upload_id(
        &rel_path,
        request.size_bytes,
        chunk_size,
        request.last_modified_ms,
    );
    let session_dir = upload_chunk_session_dir(&upload_root, &upload_id)?;

    let existing_meta = if session_dir.exists() {
        read_chunk_session_meta(&session_dir).ok()
    } else {
        None
    };

    let meta = UploadChunkSessionMeta {
        upload_id: upload_id.clone(),
        rel_path: rel_path.clone(),
        file_name: request.file_name,
        size_bytes: request.size_bytes,
        chunk_size,
        total_chunks,
        last_modified_ms: request.last_modified_ms,
    };

    if let Some(existing) = existing_meta.as_ref() {
        let stale = existing.rel_path != meta.rel_path
            || existing.size_bytes != meta.size_bytes
            || existing.chunk_size != meta.chunk_size
            || existing.total_chunks != meta.total_chunks
            || existing.last_modified_ms != meta.last_modified_ms;
        if stale {
            fs::remove_dir_all(&session_dir).map_err(|error| {
                AppError::msg(format!("failed to reset upload session: {error}"))
            })?;
        }
    }

    fs::create_dir_all(&session_dir)
        .map_err(|error| AppError::msg(format!("failed to create upload session: {error}")))?;
    write_chunk_session_meta(&session_dir, &meta)?;
    let uploaded_chunks = list_uploaded_chunk_indexes(&session_dir)?;

    Ok(Json(json!({
        "ok": true,
        "uploadId": upload_id,
        "path": rel_path,
        "chunkSize": chunk_size,
        "totalChunks": total_chunks,
        "uploadedChunks": uploaded_chunks,
    })))
}

pub async fn upload_chunk_status_get(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadChunkStatusQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    let session_dir = upload_chunk_session_dir(&upload_root, &query.upload_id)?;
    let meta = read_chunk_session_meta(&session_dir)?;
    let uploaded_chunks = list_uploaded_chunk_indexes(&session_dir)?;
    Ok(Json(json!({
        "ok": true,
        "uploadId": meta.upload_id,
        "path": meta.rel_path,
        "chunkSize": meta.chunk_size,
        "totalChunks": meta.total_chunks,
        "uploadedChunks": uploaded_chunks,
    })))
}

pub async fn upload_chunk_put(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadChunkPutQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    let session_dir = upload_chunk_session_dir(&upload_root, &query.upload_id)?;
    let meta = read_chunk_session_meta(&session_dir)?;
    if query.index >= meta.total_chunks {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "upload chunk index out of range",
        ));
    }
    if body.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "empty upload chunk",
        ));
    }
    let expected_len = expected_chunk_len(&meta, query.index);
    if body.len() != expected_len {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "unexpected chunk size: expected {expected_len} bytes, got {} bytes",
                body.len()
            ),
        ));
    }

    fs::write(upload_chunk_part_path(&session_dir, query.index), body.as_ref())
        .map_err(|error| AppError::msg(format!("failed to write upload chunk: {error}")))?;

    Ok(Json(json!({
        "ok": true,
        "uploadId": meta.upload_id,
        "index": query.index,
        "receivedBytes": expected_len,
    })))
}

pub async fn upload_chunk_complete_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadChunkCompleteRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;
    let session_dir = upload_chunk_session_dir(&upload_root, &request.upload_id)?;
    let meta = read_chunk_session_meta(&session_dir)?;
    let target = resolve_upload_target(&upload_root, &meta.rel_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AppError::msg(format!("failed to create upload parent: {error}")))?;
    }

    let temp_name = format!(
        ".{}.uploading",
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload".to_string())
    );
    let temp_target = target.with_file_name(temp_name);
    let file = fs::File::create(&temp_target)
        .map_err(|error| AppError::msg(format!("failed to create upload temp file: {error}")))?;
    let mut writer = BufWriter::new(file);
    let mut written_bytes = 0u64;

    for index in 0..meta.total_chunks {
        let chunk_path = upload_chunk_part_path(&session_dir, index);
        let chunk = fs::read(&chunk_path)
            .map_err(|error| AppError::msg(format!("failed to read upload chunk: {error}")))?;
        if chunk.len() != expected_chunk_len(&meta, index) {
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!("upload chunk {index} is incomplete"),
            ));
        }
        writer
            .write_all(&chunk)
            .map_err(|error| AppError::msg(format!("failed to append upload chunk: {error}")))?;
        written_bytes += chunk.len() as u64;
    }

    writer
        .flush()
        .map_err(|error| AppError::msg(format!("failed to flush upload file: {error}")))?;

    if written_bytes != meta.size_bytes {
        let _ = fs::remove_file(&temp_target);
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "assembled upload size mismatch: expected {} bytes, got {written_bytes} bytes",
                meta.size_bytes
            ),
        ));
    }

    fs::rename(&temp_target, &target)
        .map_err(|error| AppError::msg(format!("failed to finalize upload file: {error}")))?;
    let _ = fs::remove_dir_all(&session_dir);

    Ok(Json(json!({
        "ok": true,
        "path": meta.rel_path,
        "sizeBytes": meta.size_bytes,
    })))
}

pub async fn upload_dir_create_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadDirCreateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;

    let rel = sanitize_upload_rel(&request.path)?;
    let target = resolve_upload_target(&upload_root, &rel)?;
    if target.exists() {
        if target.is_dir() {
            return Ok(Json(json!({ "ok": true, "path": rel, "isDir": true })));
        }
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "target upload path already exists as a file",
        ));
    }

    fs::create_dir_all(&target)
        .map_err(|error| AppError::msg(format!("failed to create upload dir: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": rel, "isDir": true })))
}

pub async fn upload_file_delete(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadDeleteQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    let target = resolve_existing_upload_file(&upload_root, &query.path)?;
    if target.is_dir() {
        fs::remove_dir_all(&target)
            .map_err(|error| AppError::msg(format!("failed to remove upload dir: {error}")))?;
    } else {
        fs::remove_file(&target)
            .map_err(|error| AppError::msg(format!("failed to remove upload file: {error}")))?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn upload_file_move_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadMoveRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;

    let from_rel = sanitize_upload_rel(&request.from_path)?;
    let source = resolve_existing_upload_file(&upload_root, &from_rel)?;
    if !source.is_file() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "only files can be moved with this endpoint",
        ));
    }

    let target_rel = build_move_target_rel(&from_rel, request.to_dir.as_deref())?;
    if target_rel == from_rel {
        return Ok(Json(json!({ "ok": true, "path": from_rel })));
    }

    let target = resolve_upload_target(&upload_root, &target_rel)?;
    if target.exists() {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "target upload file already exists",
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AppError::msg(format!("failed to create target dir: {error}")))?;
    }

    fs::rename(&source, &target)
        .map_err(|error| AppError::msg(format!("failed to move upload file: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": target_rel })))
}

pub async fn upload_entry_rename_post(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadRenameRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| AppError::msg(format!("failed to create upload root: {error}")))?;

    let from_rel = sanitize_upload_rel(&request.from_path)?;
    let source = resolve_existing_upload_file(&upload_root, &from_rel)?;
    let source_is_dir = source.is_dir();
    let target_rel = build_rename_target_rel(&from_rel, &request.to_path)?;
    if target_rel == from_rel {
        return Ok(Json(json!({
            "ok": true,
            "path": from_rel,
            "isDir": source_is_dir,
        })));
    }

    let target = resolve_upload_target(&upload_root, &target_rel)?;
    if target.exists() {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "target upload path already exists",
        ));
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AppError::msg(format!("failed to create target dir: {error}")))?;
    }

    fs::rename(&source, &target)
        .map_err(|error| AppError::msg(format!("failed to rename upload entry: {error}")))?;
    Ok(Json(json!({
        "ok": true,
        "path": target_rel,
        "isDir": source_is_dir,
    })))
}

#[cfg(test)]
mod tests {
    use super::build_rename_target_rel;

    #[test]
    fn build_rename_target_rel_accepts_full_target_path() {
        let target = build_rename_target_rel("media/archive/demo.csv", "archive/2026/renamed.csv")
            .expect("build rename target");
        assert_eq!(target, "archive/2026/renamed.csv");
    }
}
