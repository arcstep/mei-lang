use std::fs;

use axum::{
    extract::{Json as AxumJson, Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::api_error::ApiError;
use crate::state::SharedState;

use super::types::*;
use super::path::*;

pub async fn upload_file_delete(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadDeleteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    let target = resolve_existing_upload_file(&upload_root, &query.path)?;
    if target.is_dir() {
        fs::remove_dir_all(&target)
            .map_err(|error| ApiError::msg(format!("failed to remove upload dir: {error}")))?;
    } else {
        fs::remove_file(&target)
            .map_err(|error| ApiError::msg(format!("failed to remove upload file: {error}")))?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn upload_file_move_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadMoveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;

    let from_rel = sanitize_upload_rel(&request.from_path)?;
    let source = resolve_existing_upload_file(&upload_root, &from_rel)?;
    if !source.is_file() {
        return Err(ApiError::status(
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
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "target upload file already exists",
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ApiError::msg(format!("failed to create target dir: {error}")))?;
    }

    fs::rename(&source, &target)
        .map_err(|error| ApiError::msg(format!("failed to move upload file: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": target_rel })))
}

pub async fn upload_entry_rename_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadRenameRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;

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
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "target upload path already exists",
        ));
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ApiError::msg(format!("failed to create target dir: {error}")))?;
    }

    fs::rename(&source, &target)
        .map_err(|error| ApiError::msg(format!("failed to rename upload entry: {error}")))?;
    Ok(Json(json!({
        "ok": true,
        "path": target_rel,
        "isDir": source_is_dir,
    })))
}
