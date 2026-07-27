use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use axum::{
    body::{Body, Bytes},
    extract::{Json as AxumJson, Path as AxumPath, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::Response,
    Json,
};
use serde_json::json;

use crate::api_error::ApiError;
use crate::state::SharedState;
use crate::upload_support::content_type_for_path;

use super::path::*;
use super::types::*;

pub async fn upload_dir_create_post(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    AxumJson(request): AxumJson<UploadDirCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    fs::create_dir_all(&upload_root)
        .map_err(|error| ApiError::msg(format!("failed to create upload root: {error}")))?;

    let rel = sanitize_upload_rel(&request.path)?;
    let target = resolve_upload_target(&upload_root, &rel)?;
    if target.exists() {
        if target.is_dir() {
            return Ok(Json(json!({ "ok": true, "path": rel, "isDir": true })));
        }
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "target upload path already exists as a file",
        ));
    }

    fs::create_dir_all(&target)
        .map_err(|error| ApiError::msg(format!("failed to create upload dir: {error}")))?;
    Ok(Json(json!({ "ok": true, "path": rel, "isDir": true })))
}

pub(super) fn percent_encode_for_content_disposition(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

pub(super) fn ascii_content_disposition_filename(file_name: &str) -> String {
    let fallback = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if fallback.trim().is_empty() {
        "download".to_string()
    } else {
        fallback
    }
}

pub(super) fn content_disposition_attachment(file_name: &str) -> Result<HeaderValue, ApiError> {
    let ascii_name = ascii_content_disposition_filename(file_name);
    let encoded = percent_encode_for_content_disposition(file_name);
    let value = format!("attachment; filename=\"{ascii_name}\"; filename*=UTF-8''{encoded}");
    HeaderValue::from_str(&value)
        .map_err(|error| ApiError::msg(format!("invalid download header: {error}")))
}

pub(super) fn content_disposition_inline(file_name: &str) -> Result<HeaderValue, ApiError> {
    let ascii_name = ascii_content_disposition_filename(file_name);
    let encoded = percent_encode_for_content_disposition(file_name);
    let value = format!("inline; filename=\"{ascii_name}\"; filename*=UTF-8''{encoded}");
    HeaderValue::from_str(&value)
        .map_err(|error| ApiError::msg(format!("invalid inline header: {error}")))
}

pub(super) fn upload_supports_inline_preview(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("pdf")
            | Some("mp4")
            | Some("webm")
            | Some("mov")
            | Some("m4v")
            | Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("webp")
            | Some("gif")
    )
}

pub(super) fn upload_file_stem_matches_basename(file_name: &str, basename: &str) -> bool {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .trim();
    let base = basename.trim();
    if stem.is_empty() || base.is_empty() {
        return false;
    }
    if stem == base {
        return true;
    }
    stem.starts_with(&format!("{base}-")) || stem.starts_with(&format!("{base}."))
}

pub(super) fn resolve_upload_file_by_basename(
    upload_root: &Path,
    dir_rel: &str,
    basename: &str,
) -> Result<PathBuf, ApiError> {
    let dir_rel = sanitize_upload_rel(dir_rel)?;
    let canonical_root = canonical_upload_root(upload_root)?;
    let dir_path = canonical_root.join(&dir_rel);
    if !dir_path.is_dir() {
        return Err(ApiError::msg(format!(
            "upload directory not found: {dir_rel}"
        )));
    }
    let mut matches = Vec::new();
    for entry in fs::read_dir(&dir_path)
        .map_err(|error| ApiError::msg(format!("failed to read upload directory: {error}")))?
    {
        let entry = entry
            .map_err(|error| ApiError::msg(format!("failed to read upload entry: {error}")))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if upload_file_stem_matches_basename(&file_name, basename) {
            matches.push(path);
        }
    }
    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        ApiError::msg(format!(
            "upload file not found for basename `{basename}` in `{dir_rel}`"
        ))
    })
}

pub(super) fn resolve_upload_download_target(
    upload_root: &Path,
    query: &UploadDownloadQuery,
) -> Result<PathBuf, ApiError> {
    let rel = sanitize_upload_rel(&query.path)?;
    if !query.match_basename {
        return resolve_existing_upload_file(upload_root, &rel);
    }
    if let Ok(path) = resolve_existing_upload_file(upload_root, &rel) {
        if path.is_dir() {
            let basename = query
                .basename
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "basename required"))?;
            return resolve_upload_file_by_basename(upload_root, &rel, basename);
        }
        return Ok(path);
    }
    let basename = query
        .basename
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            Path::new(&rel)
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "basename required"))?;
    let dir_rel = if query.basename.is_some() {
        rel.trim_end_matches('/').to_string()
    } else {
        Path::new(&rel)
            .parent()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
            .to_string()
    };
    resolve_upload_file_by_basename(upload_root, &dir_rel, &basename)
}

pub(super) fn download_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("xls") => "application/vnd.ms-excel",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("doc") => "application/msword",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("zip") | Some("rar") | Some("7z") | Some("gz") | Some("tar") => {
            "application/octet-stream"
        }
        _ => content_type_for_path(path),
    }
}

/// Parse a single `bytes=start-end` / `bytes=start-` Range. Multipart ranges are ignored.
pub(super) fn parse_bytes_range(headers: &HeaderMap, file_len: u64) -> Option<(u64, u64)> {
    if file_len == 0 {
        return None;
    }
    let raw = headers.get(RANGE)?.to_str().ok()?.trim();
    let spec = raw.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (start_raw, end_raw) = spec.split_once('-')?;
    if start_raw.is_empty() {
        let suffix = end_raw.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = file_len.saturating_sub(suffix);
        return Some((start, file_len - 1));
    }
    let start = start_raw.parse::<u64>().ok()?;
    if start >= file_len {
        return None;
    }
    let end = if end_raw.is_empty() {
        file_len - 1
    } else {
        end_raw.parse::<u64>().ok()?.min(file_len - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

fn stream_file_bytes(path: PathBuf, start: u64, length: u64) -> Body {
    Body::from_stream(async_stream::stream! {
        let mut file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) => {
                yield Err(std::io::Error::other(error.to_string()));
                return;
            }
        };
        if let Err(error) = file.seek(SeekFrom::Start(start)) {
            yield Err(std::io::Error::other(error.to_string()));
            return;
        }
        let mut remaining = length;
        let mut buf = vec![0u8; 1024 * 1024];
        while remaining > 0 {
            let chunk = remaining.min(buf.len() as u64) as usize;
            let read = match file.read(&mut buf[..chunk]) {
                Ok(read) => read,
                Err(error) => {
                    yield Err(std::io::Error::other(error.to_string()));
                    return;
                }
            };
            if read == 0 {
                break;
            }
            remaining = remaining.saturating_sub(read as u64);
            yield Ok(Bytes::from(buf[..read].to_vec()));
        }
    })
}

pub async fn upload_file_download_get(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadDownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let upload_root = resolve_upload_root_for_app(&state, &app_id)?;
    let rel = sanitize_upload_rel(&query.path)?;
    let target = resolve_upload_download_target(&upload_root, &query)?;
    if target.is_dir() {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "cannot download a directory",
        ));
    }
    let metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("failed to stat upload file: {error}")))?;
    let file_name = file_name_from_upload_rel(&rel)?;
    let content_type = download_content_type(&target);
    let file_len = metadata.len();
    let disposition = if query.inline && upload_supports_inline_preview(&target) {
        content_disposition_inline(&file_name)?
    } else {
        content_disposition_attachment(&file_name)?
    };

    let accept_ranges = HeaderValue::from_static("bytes");
    if let Some((start, end)) = parse_bytes_range(&headers, file_len) {
        let length = end - start + 1;
        let mut response = Response::new(stream_file_bytes(target.clone(), start, length));
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
        response.headers_mut().insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&length.to_string())
                .map_err(|error| ApiError::msg(format!("invalid content length: {error}")))?,
        );
        response.headers_mut().insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{file_len}"))
                .map_err(|error| ApiError::msg(format!("invalid content range: {error}")))?,
        );
        response.headers_mut().insert(
            axum::http::header::ACCEPT_RANGES,
            accept_ranges,
        );
        response
            .headers_mut()
            .insert(CONTENT_DISPOSITION, disposition);
        return Ok(response);
    }

    if headers.get(RANGE).is_some() && file_len > 0 {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
        response.headers_mut().insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes */{file_len}"))
                .map_err(|error| ApiError::msg(format!("invalid content range: {error}")))?,
        );
        response.headers_mut().insert(
            axum::http::header::ACCEPT_RANGES,
            accept_ranges,
        );
        return Ok(response);
    }

    let mut response = Response::new(stream_file_bytes(target.clone(), 0, file_len));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&file_len.to_string())
            .map_err(|error| ApiError::msg(format!("invalid content length: {error}")))?,
    );
    response.headers_mut().insert(
        axum::http::header::ACCEPT_RANGES,
        accept_ranges,
    );
    response
        .headers_mut()
        .insert(CONTENT_DISPOSITION, disposition);
    Ok(response)
}
