use std::fs;

use axum::{
    extract::{Json as AxumJson, Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::{AppError, AppState};

use super::types::*;
use super::path::*;

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
    use std::path::Path;

    use super::{
        ascii_content_disposition_filename, build_rename_target_rel,
        content_disposition_attachment, content_disposition_inline,
        percent_encode_for_content_disposition, resolve_upload_download_target,
        upload_file_stem_matches_basename, upload_supports_inline_preview, UploadDownloadQuery,
    };

    #[test]
    pub(super) fn content_disposition_supports_utf8_filename() {
        let header = content_disposition_attachment("11.预警清单.xlsx").expect("header");
        let value = header.to_str().expect("header str");
        assert!(value.contains("filename=\"11.____.xlsx\""));
        assert!(value.contains("filename*=UTF-8''"));
        assert!(percent_encode_for_content_disposition("预警").contains("%E9%A2%84"));
        assert_eq!(
            ascii_content_disposition_filename("11.预警清单.xlsx"),
            "11.____.xlsx"
        );
    }

    #[test]
    pub(super) fn content_disposition_inline_uses_inline_disposition() {
        let header = content_disposition_inline("demo.pdf").expect("header");
        let value = header.to_str().expect("header str");
        assert!(value.starts_with("inline;"));
    }

    #[test]
    pub(super) fn upload_supports_inline_preview_for_media_and_pdf() {
        assert!(upload_supports_inline_preview(Path::new(
            "文件附件/demo.pdf"
        )));
        assert!(upload_supports_inline_preview(Path::new(
            "videos/demo.mp4"
        )));
        assert!(upload_supports_inline_preview(Path::new(
            "预警摘要图片/demo.png"
        )));
        assert!(!upload_supports_inline_preview(Path::new(
            "文件附件/demo.xlsx"
        )));
    }

    #[test]
    pub(super) fn upload_file_stem_matches_basename_ignores_suffix_variants() {
        assert!(upload_file_stem_matches_basename(
            "xzzf20251105_cgj_143859.mp4",
            "xzzf20251105_cgj_143859"
        ));
        assert!(upload_file_stem_matches_basename(
            "xzzf20251105_cgj_143859-1.png",
            "xzzf20251105_cgj_143859"
        ));
        assert!(upload_file_stem_matches_basename(
            "xzzf20251105_cgj_143859.jpg",
            "xzzf20251105_cgj_143859"
        ));
        assert!(!upload_file_stem_matches_basename(
            "other.mp4",
            "xzzf20251105_cgj_143859"
        ));
    }

    #[test]
    pub(super) fn upload_download_match_basename_searches_directory_path() {
        let upload_root = std::env::temp_dir().join(format!(
            "mei-upload-download-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&upload_root);
        let image_dir = upload_root.join("预警摘要图片");
        std::fs::create_dir_all(&image_dir).expect("image dir");
        let image_path = image_dir.join("xzzf20251105_cgj_143859-1.png");
        std::fs::write(&image_path, b"png").expect("image");

        let target = resolve_upload_download_target(
            upload_root.as_path(),
            &UploadDownloadQuery {
                path: "预警摘要图片".to_string(),
                inline: true,
                match_basename: true,
                basename: Some("xzzf20251105_cgj_143859".to_string()),
            },
        )
        .expect("resolved basename");

        assert!(target.is_file());
        assert_eq!(
            target.file_name().and_then(|value| value.to_str()),
            Some("xzzf20251105_cgj_143859-1.png")
        );
        let _ = std::fs::remove_dir_all(upload_root);
    }

    #[test]
    pub(super) fn build_rename_target_rel_accepts_full_target_path() {
        let target = build_rename_target_rel("media/archive/demo.csv", "archive/2026/renamed.csv")
            .expect("build rename target");
        assert_eq!(target, "archive/2026/renamed.csv");
    }
}
