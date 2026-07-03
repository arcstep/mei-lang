use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::SharedState;

const LIBRARY_FILE: &str = ".mei-presentation-library.json";
const SCRIPT_SUFFIX: &str = ".presentation.mdx";
const MAX_SCRIPT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PresentationLibraryDocument {
    #[serde(rename = "schemaVersion", default)]
    schema_version: u32,
    #[serde(rename = "defaultScriptId", default)]
    default_script_id: String,
}

#[derive(Debug, Serialize)]
struct PresentationScriptEntry {
    id: String,
    title: String,
    path: String,
    #[serde(rename = "modifiedMs", skip_serializing_if = "Option::is_none")]
    modified_ms: Option<i64>,
    #[serde(rename = "isDefault", default)]
    is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct PutPresentationScriptRequest {
    pub source: String,
    #[serde(default)]
    pub title: Option<String>,
}

fn sanitize_script_id(raw: &str) -> Result<String, (StatusCode, String)> {
    let id = raw.trim();
    if id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "scriptId 不能为空".to_string()));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "scriptId 仅允许字母、数字、_ 和 -".to_string(),
        ));
    }
    Ok(id.to_string())
}

fn sanitize_rel_dir(raw: &str) -> Result<String, (StatusCode, String)> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    for component in Path::new(&trimmed).components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "演说稿路径不能越出演说稿目录".to_string(),
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(trimmed.trim_start_matches('/').to_string())
}

pub fn default_presentation_rel_path() -> &'static str {
    "src/presentation"
}

pub fn resolve_presentation_root(
    workspace_root: &Path,
    app_id: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let rel = config
        .paths
        .presentation
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_presentation_rel_path());
    Ok(app_root.join(rel))
}

fn library_path(root: &Path) -> PathBuf {
    root.join(LIBRARY_FILE)
}

fn read_library(root: &Path) -> PresentationLibraryDocument {
    let path = library_path(root);
    if !path.is_file() {
        return PresentationLibraryDocument::default();
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_library(root: &Path, library: &PresentationLibraryDocument) -> Result<(), (StatusCode, String)> {
    fs::create_dir_all(root).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("创建演说稿目录失败: {error}"),
        )
    })?;
    let path = library_path(root);
    let payload = serde_json::to_string_pretty(library).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("序列化演说稿库失败: {error}"),
        )
    })?;
    fs::write(&path, payload).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("写入演说稿库元数据失败: {error}"),
        )
    })
}

fn script_id_from_file_name(file_name: &str) -> Option<String> {
    let name = file_name.trim();
    let stem = name.strip_suffix(SCRIPT_SUFFIX)?;
    if stem.is_empty() {
        return None;
    }
    sanitize_script_id(stem).ok()
}

fn script_file_name(script_id: &str) -> String {
    format!("{script_id}{SCRIPT_SUFFIX}")
}

fn script_rel_path(script_id: &str) -> String {
    script_file_name(script_id)
}

fn resolve_script_path(root: &Path, script_id: &str) -> Result<PathBuf, (StatusCode, String)> {
    let rel = sanitize_rel_dir(&script_rel_path(script_id))?;
    let canonical_root = root.canonicalize().map_err(|error| {
        (
            StatusCode::NOT_FOUND,
            format!("演说稿目录不可用: {error}"),
        )
    })?;
    let target = root.join(&rel);
    if target.exists() {
        let canonical_target = target.canonicalize().map_err(|error| {
            (
                StatusCode::NOT_FOUND,
                format!("演说稿路径不可用: {error}"),
            )
        })?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err((StatusCode::BAD_REQUEST, "演说稿路径越界".to_string()));
        }
        return Ok(canonical_target);
    }
    for component in Path::new(&rel).components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err((StatusCode::BAD_REQUEST, "演说稿路径越界".to_string()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(target)
}

fn system_time_to_epoch_ms(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as i64)
}

fn parse_title_from_source(source: &str) -> Option<String> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("title:") {
            let title = value.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn read_script_title(path: &Path, source: &str) -> String {
    parse_title_from_source(source)
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "未命名演说稿".to_string())
}

fn list_script_entries(root: &Path, default_script_id: &str) -> Result<Vec<PresentationScriptEntry>, (StatusCode, String)> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(root).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取演说稿目录失败: {error}"),
        )
    })?;
    for item in read_dir.flatten() {
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(script_id) = script_id_from_file_name(file_name) else {
            continue;
        };
        let source = fs::read_to_string(&path).unwrap_or_default();
        let modified_ms = item
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_epoch_ms);
        entries.push(PresentationScriptEntry {
            id: script_id.clone(),
            title: read_script_title(&path, &source),
            path: script_rel_path(&script_id),
            modified_ms,
            is_default: default_script_id == script_id,
        });
    }
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(entries)
}

pub fn read_default_script_id(workspace_root: &Path, app_id: &str) -> Option<String> {
    let root = resolve_presentation_root(workspace_root, app_id).ok()?;
    let library = read_library(&root);
    let configured = library.default_script_id.trim();
    if !configured.is_empty() {
        let path = resolve_script_path(&root, configured).ok()?;
        if path.is_file() {
            return Some(configured.to_string());
        }
    }
    for candidate in ["intro", "default", "main"] {
        let path = resolve_script_path(&root, candidate).ok()?;
        if path.is_file() {
            return Some(candidate.to_string());
        }
    }
    list_script_entries(&root, "")
        .ok()
        .and_then(|entries| entries.first().map(|entry| entry.id.clone()))
}

pub async fn api_list_presentation_scripts(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let app_id = app_id.trim();
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let root = match resolve_presentation_root(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let library = read_library(&root);
    let default_script_id = read_default_script_id(workspace_root.as_path(), app_id)
        .unwrap_or_else(|| library.default_script_id.clone());
    let scripts = match list_script_entries(&root, &default_script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    Json(json!({
        "appId": app_id,
        "root": root.strip_prefix(resolve_app_root(workspace_root.as_path(), app_id))
            .ok()
            .and_then(|value| value.to_str())
            .unwrap_or(default_presentation_rel_path()),
        "defaultScriptId": default_script_id,
        "scripts": scripts,
    }))
    .into_response()
}

pub async fn api_get_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let root = match resolve_presentation_root(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let path = match resolve_script_path(&root, &script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if !path.is_file() {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在"),
        );
    }
    let source = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("读取演说稿失败: {error}"),
            )
        }
    };
    let default_script_id = read_default_script_id(workspace_root.as_path(), app_id)
        .unwrap_or_default();
    Json(json!({
        "appId": app_id,
        "id": script_id,
        "path": script_rel_path(&script_id),
        "title": read_script_title(&path, &source),
        "source": source,
        "isDefault": default_script_id == script_id,
    }))
    .into_response()
}

pub async fn api_put_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
    Json(request): Json<PutPresentationScriptRequest>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let source = request.source.trim();
    if source.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "source 不能为空");
    }
    if source.len() > MAX_SCRIPT_BYTES {
        return error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("演说稿超过 {} 字节上限", MAX_SCRIPT_BYTES),
        );
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let root = match resolve_presentation_root(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Err(error) = fs::create_dir_all(&root) {
        return error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("创建演说稿目录失败: {error}"),
        );
    }
    let path = match resolve_script_path(&root, &script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Err(error) = fs::write(&path, format!("{source}\n")) {
        return error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("保存演说稿失败: {error}"),
        );
    }
    let mut library = read_library(&root);
    if library.default_script_id.trim().is_empty() {
        library.default_script_id = script_id.clone();
        library.schema_version = 1;
        if let Err((status, message)) = write_library(&root, &library) {
            return error_json(status, message);
        }
    }
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| read_script_title(&path, source));
    Json(json!({
        "ok": true,
        "appId": app_id,
        "id": script_id,
        "path": script_rel_path(&script_id),
        "title": title,
        "isDefault": library.default_script_id == script_id,
    }))
    .into_response()
}

pub async fn api_set_default_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let root = match resolve_presentation_root(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let path = match resolve_script_path(&root, &script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if !path.is_file() {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在"),
        );
    }
    let mut library = read_library(&root);
    library.schema_version = 1;
    library.default_script_id = script_id.clone();
    if let Err((status, message)) = write_library(&root, &library) {
        return error_json(status, message);
    }
    Json(json!({
        "ok": true,
        "appId": app_id,
        "defaultScriptId": script_id,
    }))
    .into_response()
}

fn forbidden(app_id: &str) -> Response {
    error_json(
        StatusCode::FORBIDDEN,
        format!("当前账号无权访问 app `{app_id}`"),
    )
}

fn error_json(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "ok": false,
            "error": message.into(),
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_id_from_file_name_parses_presentation_mdx() {
        assert_eq!(
            script_id_from_file_name("intro.presentation.mdx").as_deref(),
            Some("intro")
        );
        assert!(script_id_from_file_name("intro.mdx").is_none());
    }

    #[test]
    fn parse_title_from_source_reads_frontmatter() {
        let source = "---\ntitle: 迷你公园导览\npresentation: intro\n---\n";
        assert_eq!(
            parse_title_from_source(source).as_deref(),
            Some("迷你公园导览")
        );
    }
}
