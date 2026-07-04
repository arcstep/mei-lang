use std::{
    fs,
    io::{BufWriter, Write},
};

use axum::{
    body::Bytes,
    extract::{Json as AxumJson, Multipart, Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::api_error::ApiError;
use crate::state::SharedState;
use crate::upload_support::invalidate_after_upload;

use super::types::*;
use super::path::*;

pub async fn upload_file_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    std::fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;
    let mut upload_dir: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::msg(format!("multipart read failed: {error}")))?
    {
        match field.name() {
            Some("dir") => {
                upload_dir =
                    Some(field.text().await.map_err(|error| {
                        ApiError::msg(format!("read upload dir failed: {error}"))
                    })?);
            }
            Some("file") => {
                file_name = field.file_name().map(str::to_string);
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| {
                            ApiError::msg(format!("read upload bytes failed: {error}"))
                        })?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }
    let file_name = file_name
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "missing upload file"))?;
    let file_bytes = file_bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "empty upload file"))?;
    let rel = build_upload_rel(upload_dir.as_deref(), &file_name)?;
    let target = resolve_upload_target(&upload_root, &rel)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ApiError::msg(format!("failed to create upload parent: {error}")))?;
    }
    fs::write(&target, file_bytes)
        .map_err(|error| ApiError::msg(format!("failed to write upload file: {error}")))?;
    invalidate_after_upload(&state, app_id.as_str());
    Ok(Json(json!({
        "ok": true,
        "path": rel,
        "cacheInvalidated": true,
        "compileCacheCleared": false,
    })))
}

pub async fn upload_chunk_init_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadChunkInitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;

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
                ApiError::msg(format!("failed to reset upload session: {error}"))
            })?;
        }
    }

    fs::create_dir_all(&session_dir)
        .map_err(|error| ApiError::msg(format!("failed to create upload session: {error}")))?;
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
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadChunkStatusQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
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
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadChunkPutQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    let session_dir = upload_chunk_session_dir(&upload_root, &query.upload_id)?;
    let meta = read_chunk_session_meta(&session_dir)?;
    if query.index >= meta.total_chunks {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "upload chunk index out of range",
        ));
    }
    if body.is_empty() {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "empty upload chunk",
        ));
    }
    let expected_len = expected_chunk_len(&meta, query.index);
    if body.len() != expected_len {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "unexpected chunk size: expected {expected_len} bytes, got {} bytes",
                body.len()
            ),
        ));
    }

    fs::write(
        upload_chunk_part_path(&session_dir, query.index),
        body.as_ref(),
    )
    .map_err(|error| ApiError::msg(format!("failed to write upload chunk: {error}")))?;

    Ok(Json(json!({
        "ok": true,
        "uploadId": meta.upload_id,
        "index": query.index,
        "receivedBytes": expected_len,
    })))
}

pub async fn upload_chunk_complete_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadChunkCompleteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;
    let session_dir = upload_chunk_session_dir(&upload_root, &request.upload_id)?;
    let meta = read_chunk_session_meta(&session_dir)?;
    let target = resolve_upload_target(&upload_root, &meta.rel_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ApiError::msg(format!("failed to create upload parent: {error}")))?;
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
        .map_err(|error| ApiError::msg(format!("failed to create upload temp file: {error}")))?;
    let mut writer = BufWriter::new(file);
    let mut written_bytes = 0u64;

    for index in 0..meta.total_chunks {
        let chunk_path = upload_chunk_part_path(&session_dir, index);
        let chunk = fs::read(&chunk_path)
            .map_err(|error| ApiError::msg(format!("failed to read upload chunk: {error}")))?;
        if chunk.len() != expected_chunk_len(&meta, index) {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                format!("upload chunk {index} is incomplete"),
            ));
        }
        writer
            .write_all(&chunk)
            .map_err(|error| ApiError::msg(format!("failed to append upload chunk: {error}")))?;
        written_bytes += chunk.len() as u64;
    }

    writer
        .flush()
        .map_err(|error| ApiError::msg(format!("failed to flush upload file: {error}")))?;

    if written_bytes != meta.size_bytes {
        let _ = fs::remove_file(&temp_target);
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "assembled upload size mismatch: expected {} bytes, got {written_bytes} bytes",
                meta.size_bytes
            ),
        ));
    }

    fs::rename(&temp_target, &target)
        .map_err(|error| ApiError::msg(format!("failed to finalize upload file: {error}")))?;
    let _ = fs::remove_dir_all(&session_dir);
    invalidate_after_upload(&state, app_id.as_str());
    Ok(Json(json!({
        "ok": true,
        "path": meta.rel_path,
        "sizeBytes": meta.size_bytes,
        "cacheInvalidated": true,
        "compileCacheCleared": false,
    })))
}

