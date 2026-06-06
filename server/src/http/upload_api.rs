use std::path::{Component, Path, PathBuf};

use axum::{
    extract::{Multipart, Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::load_mei_config_for_app;
use serde::Deserialize;
use serde_json::json;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct UploadDeleteQuery {
    pub path: String,
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
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "app has no paths.upload configured"))?;
    Ok(app_root.join(rel))
}

fn sanitize_upload_rel(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err(AppError::status(StatusCode::BAD_REQUEST, "empty upload path"));
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

fn resolve_upload_file(upload_root: &Path, rel: &str) -> Result<PathBuf, AppError> {
    let rel = sanitize_upload_rel(rel)?;
    let resolved = upload_root.join(&rel);
    let canonical_root = upload_root
        .canonicalize()
        .map_err(|error| AppError::msg(format!("upload root unavailable: {error}")))?;
    let canonical_file = resolved
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
                upload_dir = Some(
                    field
                        .text()
                        .await
                        .map_err(|error| AppError::msg(format!("read upload dir failed: {error}")))?,
                );
            }
            Some("file") => {
                file_name = field.file_name().map(str::to_string);
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| AppError::msg(format!("read upload bytes failed: {error}")))?
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
    let rel = if let Some(dir) = upload_dir.as_deref() {
        let dir = sanitize_upload_rel(dir)?;
        format!("{dir}/{}", sanitize_upload_rel(&file_name)?)
    } else {
        sanitize_upload_rel(&file_name)?
    };
    let target = resolve_upload_file(&upload_root, &rel)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AppError::msg(format!("failed to create upload parent: {error}")))?;
    }
    std::fs::write(&target, file_bytes)
        .map_err(|error| AppError::msg(format!("failed to write upload file: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": rel })))
}

pub async fn upload_file_delete(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadDeleteQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let upload_root = resolve_upload_root(&state, &app_id)?;
    let target = resolve_upload_file(&upload_root, &query.path)?;
    if target.is_dir() {
        std::fs::remove_dir_all(&target)
            .map_err(|error| AppError::msg(format!("failed to remove upload dir: {error}")))?;
    } else {
        std::fs::remove_file(&target)
            .map_err(|error| AppError::msg(format!("failed to remove upload file: {error}")))?;
    }
    Ok(Json(json!({ "ok": true })))
}
