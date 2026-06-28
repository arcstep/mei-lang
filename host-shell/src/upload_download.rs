use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
};
use mei_lang_kernel::load_mei_config_for_app;
use serde::Deserialize;

use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub(crate) struct UploadDownloadQuery {
    path: String,
    #[serde(default)]
    inline: bool,
    #[serde(default)]
    match_basename: bool,
    basename: Option<String>,
}

fn sanitize_upload_rel(raw: &str) -> Result<String, (StatusCode, String)> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty upload path".to_string()));
    }
    for component in Path::new(&trimmed).components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "upload path must stay within upload directory".to_string(),
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(trimmed.trim_start_matches('/').to_string())
}

fn resolve_upload_root(
    workspace_root: &Path,
    app_id: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let rel = config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "app has no paths.upload configured".to_string(),
            )
        })?;
    Ok(app_root.join(rel))
}

fn canonical_upload_root(upload_root: &Path) -> Result<PathBuf, (StatusCode, String)> {
    upload_root
        .canonicalize()
        .map_err(|error| (StatusCode::NOT_FOUND, format!("upload root unavailable: {error}")))
}

fn resolve_existing_upload_file(
    upload_root: &Path,
    rel: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    let rel = sanitize_upload_rel(rel)?;
    let canonical_root = canonical_upload_root(upload_root)?;
    let canonical_file = canonical_root
        .join(&rel)
        .canonicalize()
        .map_err(|error| (StatusCode::NOT_FOUND, format!("upload file not found: {error}")))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err((
            StatusCode::BAD_REQUEST,
            "upload path escapes upload directory".to_string(),
        ));
    }
    Ok(canonical_file)
}

fn upload_file_stem_matches_basename(file_name: &str, basename: &str) -> bool {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .trim();
    let base = basename.trim();
    if stem.is_empty() || base.is_empty() {
        return false;
    }
    stem == base || stem.starts_with(&format!("{base}-")) || stem.starts_with(&format!("{base}."))
}

fn resolve_upload_file_by_basename(
    upload_root: &Path,
    dir_rel: &str,
    basename: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    let dir_rel = sanitize_upload_rel(dir_rel)?;
    let canonical_root = canonical_upload_root(upload_root)?;
    let dir_path = canonical_root.join(&dir_rel);
    if !dir_path.is_dir() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("upload directory not found: {dir_rel}"),
        ));
    }
    let mut matches = Vec::new();
    for entry in fs::read_dir(&dir_path)
        .map_err(|error| (StatusCode::NOT_FOUND, format!("failed to read upload directory: {error}")))?
    {
        let entry = entry
            .map_err(|error| (StatusCode::NOT_FOUND, format!("failed to read upload entry: {error}")))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if upload_file_stem_matches_basename(&file_name, basename) {
            matches.push(path);
        }
    }
    if matches.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("no upload file matches basename `{basename}` in `{dir_rel}`"),
        ));
    }
    if matches.len() > 1 {
        matches.sort_by_cached_key(|path| path.file_name().map(|name| name.to_owned()));
    }
    Ok(matches[0].clone())
}

fn resolve_upload_download_target(
    upload_root: &Path,
    query: &UploadDownloadQuery,
) -> Result<PathBuf, (StatusCode, String)> {
    let rel = sanitize_upload_rel(&query.path)?;
    if let Ok(path) = resolve_existing_upload_file(upload_root, &rel) {
        return Ok(path);
    }
    if query.match_basename {
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
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "basename required when match_basename is true".to_string(),
                )
            })?;
        let dir_rel = Path::new(&rel)
            .parent()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
            .to_string();
        return resolve_upload_file_by_basename(upload_root, &dir_rel, &basename);
    }
    resolve_existing_upload_file(upload_root, &rel)
}

fn content_type_for_path(path: &Path) -> &'static str {
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
        _ => "application/octet-stream",
    }
}

fn upload_supports_inline_preview(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("pdf") | Some("mp4") | Some("webm") | Some("mov") | Some("m4v") | Some("png")
            | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif")
    )
}

fn percent_encode_for_content_disposition(value: &str) -> String {
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

fn content_disposition(file_name: &str, inline: bool) -> Result<HeaderValue, (StatusCode, String)> {
    let ascii_name = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let ascii_name = if ascii_name.trim().is_empty() {
        "download".to_string()
    } else {
        ascii_name
    };
    let encoded = percent_encode_for_content_disposition(file_name);
    let kind = if inline { "inline" } else { "attachment" };
    let value = format!("{kind}; filename=\"{ascii_name}\"; filename*=UTF-8''{encoded}");
    HeaderValue::from_str(&value)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, format!("invalid header: {error}")))
}

pub async fn upload_file_download_get(
    State(state): State<SharedState>,
    AxumPath(app_id): AxumPath<String>,
    Query(query): Query<UploadDownloadQuery>,
) -> Response {
    let guard = state.read().expect("state lock");
    let upload_root = match resolve_upload_root(guard.ctx.workspace_root.as_path(), app_id.as_str()) {
        Ok(root) => root,
        Err((status, message)) => return (status, message).into_response(),
    };
    let target = match resolve_upload_download_target(&upload_root, &query) {
        Ok(path) => path,
        Err((status, message)) => return (status, message).into_response(),
    };
    if target.is_dir() {
        return (StatusCode::BAD_REQUEST, "cannot download a directory").into_response();
    }
    let bytes = match fs::read(&target) {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::NOT_FOUND,
                format!("failed to read upload file: {error}"),
            )
                .into_response();
        }
    };
    let file_name = Path::new(&query.path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download")
        .to_string();
    let inline = query.inline && upload_supports_inline_preview(&target);
    let content_type = content_type_for_path(&target);
    let disposition = match content_disposition(&file_name, inline) {
        Ok(value) => value,
        Err((status, message)) => return (status, message).into_response(),
    };
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (
                CONTENT_LENGTH,
                HeaderValue::from_str(&bytes.len().to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("0")),
            ),
            (CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolves_upload_file_under_app_upload_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app_root = workspace.join("apps").join("data-demo");
        let upload_root = app_root.join("upload");
        fs::create_dir_all(upload_root.join("文件附件")).expect("mkdir");
        fs::write(
            upload_root.join("文件附件/demo.pdf"),
            b"%PDF-1.4 test",
        )
        .expect("write pdf");
        fs::write(
            app_root.join("app.config.json"),
            r#"{"schemaVersion":1,"entry":{"main":"main.mei"},"paths":{"upload":"upload"}}"#,
        )
        .expect("write config");
        let resolved =
            resolve_upload_download_target(&upload_root, &UploadDownloadQuery {
                path: "文件附件/demo.pdf".to_string(),
                inline: true,
                match_basename: false,
                basename: None,
            })
            .expect("resolve");
        assert!(resolved.is_file());
    }
}
