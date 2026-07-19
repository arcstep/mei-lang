use std::{
    fs,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, Bytes},
    extract::{Extension, Json as AxumJson, Multipart, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::Response,
    Json,
};
use mei_host_auth::AuthPrincipal;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api_error::ApiError;
use crate::state::SharedState;

use super::download::{content_disposition_attachment, download_content_type};
use super::path::*;
use super::types::*;

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareListQuery {
    pub path: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceShareEntry {
    name: String,
    path: String,
    is_dir: bool,
    size_bytes: Option<u64>,
    modified_ms: Option<u64>,
    revision: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareDirRequest {
    pub path: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareRenameRequest {
    pub from_path: String,
    pub to_path: String,
    pub expected_revision: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareMoveRequest {
    pub from_path: String,
    pub to_dir: Option<String>,
    pub expected_revision: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareDeleteQuery {
    pub path: String,
    pub expected_revision: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareDownloadQuery {
    pub path: String,
    pub expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShareChunkInitRequest {
    pub file_name: String,
    pub dir: Option<String>,
    pub size_bytes: u64,
    pub chunk_size: usize,
    pub last_modified_ms: Option<u64>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy)]
enum SharePermission {
    View,
    Upload,
    Organize,
    Delete,
}

fn ensure_share_permission(
    principal: Option<&AuthPrincipal>,
    permission: SharePermission,
) -> Result<(), ApiError> {
    let Some(principal) = principal else {
        return Ok(());
    };
    let caps = principal.capabilities();
    let allowed = match permission {
        SharePermission::View => caps.workspace_share_view,
        SharePermission::Upload => caps.workspace_share_upload,
        SharePermission::Organize => caps.workspace_share_organize,
        SharePermission::Delete => caps.workspace_share_delete,
    };
    if allowed {
        Ok(())
    } else {
        Err(ApiError::status(
            StatusCode::FORBIDDEN,
            "workspace share permission denied",
        ))
    }
}

fn share_actor(principal: Option<&AuthPrincipal>) -> String {
    principal
        .map(|value| format!("{}:{}", value.username, value.role.as_str()))
        .unwrap_or_else(|| "auth-disabled".to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

fn append_share_audit(
    state: &SharedState,
    principal: Option<&AuthPrincipal>,
    action: &str,
    path: &str,
) -> Result<(), ApiError> {
    let workspace_root = state.read().expect("state lock").ctx.workspace_root.clone();
    let audit_dir = workspace_root
        .join(".mei")
        .join("local")
        .join("workspace-share");
    fs::create_dir_all(&audit_dir)
        .map_err(|error| ApiError::msg(format!("create workspace share audit dir: {error}")))?;
    let audit_path = audit_dir.join("audit.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .map_err(|error| ApiError::msg(format!("open workspace share audit: {error}")))?;
    let line = serde_json::to_string(&json!({
        "atMs": now_ms(),
        "actor": share_actor(principal),
        "action": action,
        "path": path,
    }))
    .map_err(|error| ApiError::msg(format!("encode workspace share audit: {error}")))?;
    writeln!(file, "{line}")
        .map_err(|error| ApiError::msg(format!("append workspace share audit: {error}")))
}

fn normalize_idempotency_key(raw: &str) -> Result<String, ApiError> {
    let key = raw.trim();
    if key.is_empty()
        || key.len() > 120
        || !key
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.'))
    {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "invalid workspace share idempotency key",
        ));
    }
    Ok(key.to_string())
}

fn share_control_dir(state: &SharedState) -> PathBuf {
    state
        .read()
        .expect("state lock")
        .ctx
        .workspace_root
        .join(".mei")
        .join("local")
        .join("workspace-share")
}

fn replay_share_receipt(
    state: &SharedState,
    key: &str,
) -> Result<Option<serde_json::Value>, ApiError> {
    let key = normalize_idempotency_key(key)?;
    let path = share_control_dir(state)
        .join("idempotency")
        .join(format!("{key}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .map_err(|error| ApiError::msg(format!("read workspace share receipt: {error}")))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| ApiError::msg(format!("decode workspace share receipt: {error}")))
}

fn store_share_receipt(
    state: &SharedState,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), ApiError> {
    let key = normalize_idempotency_key(key)?;
    let dir = share_control_dir(state).join("idempotency");
    fs::create_dir_all(&dir)
        .map_err(|error| ApiError::msg(format!("create workspace share receipt dir: {error}")))?;
    let target = dir.join(format!("{key}.json"));
    let temp = dir.join(format!(".{key}.tmp"));
    let bytes = serde_json::to_vec(value)
        .map_err(|error| ApiError::msg(format!("encode workspace share receipt: {error}")))?;
    fs::write(&temp, bytes)
        .map_err(|error| ApiError::msg(format!("write workspace share receipt: {error}")))?;
    fs::rename(&temp, &target)
        .map_err(|error| ApiError::msg(format!("commit workspace share receipt: {error}")))
}

fn ensure_share_root(state: &SharedState) -> Result<PathBuf, ApiError> {
    let root = resolve_workspace_share_root(state);
    fs::create_dir_all(&root)
        .map_err(|error| ApiError::msg(format!("create workspace share root: {error}")))?;
    Ok(root)
}

fn create_target_parent_within_root(root: &Path, target: &Path) -> Result<(), ApiError> {
    let canonical_root = canonical_upload_root(root)?;
    let parent = target
        .parent()
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "invalid workspace share path"))?;
    let relative = parent.strip_prefix(&canonical_root).map_err(|_| {
        ApiError::status(
            StatusCode::BAD_REQUEST,
            "workspace share target escapes share root",
        )
    })?;
    let mut current = canonical_root;
    for component in relative.components() {
        current.push(component);
        if current.exists() {
            let metadata = fs::symlink_metadata(&current).map_err(|error| {
                ApiError::msg(format!("read workspace share parent metadata: {error}"))
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(ApiError::status(
                    StatusCode::BAD_REQUEST,
                    "workspace share parent must be a real directory within share root",
                ));
            }
        } else {
            fs::create_dir(&current).map_err(|error| {
                ApiError::msg(format!("create workspace share parent: {error}"))
            })?;
        }
    }
    Ok(())
}

fn resolve_share_dir(root: &Path, rel: Option<&str>) -> Result<(String, PathBuf), ApiError> {
    let rel = rel.map(str::trim).filter(|value| !value.is_empty());
    match rel {
        Some(rel) => {
            let clean = sanitize_upload_rel(rel)?;
            let path = resolve_existing_upload_file(root, clean.as_str())?;
            if !path.is_dir() {
                return Err(ApiError::status(
                    StatusCode::BAD_REQUEST,
                    "workspace share path is not a directory",
                ));
            }
            Ok((clean, path))
        }
        None => Ok((String::new(), canonical_upload_root(root)?)),
    }
}

fn entry_modified_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as u64)
}

fn entry_revision(path: &str, metadata: &fs::Metadata) -> String {
    let fingerprint = format!(
        "{path}\n{}\n{}\n{}",
        metadata.len(),
        entry_modified_ms(metadata).unwrap_or_default(),
        metadata.is_dir()
    );
    let mut hash = 0xcbf29ce484222325u64;
    for byte in fingerprint.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("r{hash:016x}")
}

fn require_expected_revision(
    path: &str,
    metadata: &fs::Metadata,
    expected: &str,
) -> Result<(), ApiError> {
    if entry_revision(path, metadata) == expected.trim() {
        Ok(())
    } else {
        Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share revision conflict",
        ))
    }
}

fn scope_revision(entries: &[WorkspaceShareEntry]) -> String {
    let fingerprint = entries
        .iter()
        .map(|entry| format!("{}:{}", entry.path, entry.revision))
        .collect::<Vec<_>>()
        .join("\n");
    let mut hash = 0xcbf29ce484222325u64;
    for byte in fingerprint.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("s{hash:016x}")
}

fn list_share_entries(
    dir_rel: &str,
    dir_path: &Path,
    query: Option<&str>,
) -> Result<Vec<WorkspaceShareEntry>, ApiError> {
    let needle = query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir_path)
        .map_err(|error| ApiError::msg(format!("read workspace share directory: {error}")))?
    {
        let entry =
            entry.map_err(|error| ApiError::msg(format!("read workspace share entry: {error}")))?;
        let file_type = entry
            .file_type()
            .map_err(|error| ApiError::msg(format!("read workspace share entry type: {error}")))?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(".mei-")
            || name.ends_with(".uploading")
            || needle
                .as_ref()
                .is_some_and(|value| !name.to_lowercase().contains(value))
        {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| ApiError::msg(format!("read workspace share metadata: {error}")))?;
        let path = if dir_rel.is_empty() {
            name.clone()
        } else {
            format!("{dir_rel}/{name}")
        };
        entries.push(WorkspaceShareEntry {
            name,
            revision: entry_revision(path.as_str(), &metadata),
            path,
            is_dir: file_type.is_dir(),
            size_bytes: file_type.is_file().then_some(metadata.len()),
            modified_ms: entry_modified_ms(&metadata),
        });
    }
    entries.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(entries)
}

fn collect_share_directories(root: &Path) -> Result<Vec<String>, ApiError> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<(), ApiError> {
        for entry in fs::read_dir(current)
            .map_err(|error| ApiError::msg(format!("read workspace share tree: {error}")))?
        {
            let entry = entry.map_err(|error| {
                ApiError::msg(format!("read workspace share tree entry: {error}"))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                ApiError::msg(format!("read workspace share tree entry type: {error}"))
            })?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with(".mei-") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .map_err(|error| ApiError::msg(format!("resolve workspace share tree: {error}")))?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
            walk(root, &path, out)?;
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

pub async fn workspace_share_list_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<WorkspaceShareListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::View)?;
    let root = ensure_share_root(&state)?;
    let (path, dir) = resolve_share_dir(&root, query.path.as_deref())?;
    let entries = list_share_entries(path.as_str(), &dir, query.q.as_deref())?;
    let directories = collect_share_directories(&root)?;
    let revision = scope_revision(entries.as_slice());
    append_share_audit(
        &state,
        principal,
        "list",
        if path.is_empty() { "/" } else { path.as_str() },
    )?;
    Ok(Json(json!({
        "path": path,
        "entries": entries,
        "directories": directories,
        "revision": revision,
    })))
}

pub async fn workspace_share_entry_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<UploadDeleteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::View)?;
    let root = ensure_share_root(&state)?;
    let rel = sanitize_upload_rel(query.path.as_str())?;
    let target = resolve_existing_upload_file(&root, rel.as_str())?;
    let metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("read workspace share entry metadata: {error}")))?;
    let name = file_name_from_upload_rel(rel.as_str())?;
    append_share_audit(&state, principal, "get", rel.as_str())?;
    Ok(Json(json!({
        "entry": WorkspaceShareEntry {
            name,
            revision: entry_revision(rel.as_str(), &metadata),
            path: rel,
            is_dir: metadata.is_dir(),
            size_bytes: metadata.is_file().then_some(metadata.len()),
            modified_ms: entry_modified_ms(&metadata),
        }
    })))
}

pub async fn workspace_share_upload_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    let root = ensure_share_root(&state)?;
    let mut upload_dir: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut idempotency_key: Option<String> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::msg(format!("workspace share multipart read failed: {error}")))?
    {
        match field.name() {
            Some("dir") => {
                upload_dir = Some(field.text().await.map_err(|error| {
                    ApiError::msg(format!("read workspace share upload dir: {error}"))
                })?);
            }
            Some("file") => {
                file_name = field.file_name().map(str::to_string);
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| {
                            ApiError::msg(format!("read workspace share upload bytes: {error}"))
                        })?
                        .to_vec(),
                );
            }
            Some("idempotency_key") => {
                idempotency_key = Some(field.text().await.map_err(|error| {
                    ApiError::msg(format!("read workspace share idempotency key: {error}"))
                })?);
            }
            _ => {}
        }
    }
    let file_name = file_name
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "missing workspace share file"))?;
    let bytes = file_bytes
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, "empty workspace share file"))?;
    let idempotency_key =
        normalize_idempotency_key(idempotency_key.as_deref().ok_or_else(|| {
            ApiError::status(
                StatusCode::BAD_REQUEST,
                "workspace share idempotency key required",
            )
        })?)?;
    if let Some(receipt) = replay_share_receipt(&state, idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let rel = build_upload_rel(upload_dir.as_deref(), file_name.as_str())?;
    let target = resolve_upload_target(&root, rel.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share target already exists",
        ));
    }
    create_target_parent_within_root(&root, &target)?;
    let session_id = stable_upload_id(
        idempotency_key.as_str(),
        bytes.len() as u64,
        bytes.len().max(1),
        None,
    );
    let session_dir = upload_chunk_session_dir(&root, session_id.as_str())?;
    fs::create_dir_all(&session_dir)
        .map_err(|error| ApiError::msg(format!("create workspace share upload temp: {error}")))?;
    let temp = session_dir.join("upload.tmp");
    fs::write(&temp, bytes)
        .map_err(|error| ApiError::msg(format!("write workspace share file: {error}")))?;
    fs::rename(&temp, &target)
        .map_err(|error| ApiError::msg(format!("commit workspace share file: {error}")))?;
    let _ = fs::remove_dir_all(&session_dir);
    append_share_audit(&state, principal, "upload", rel.as_str())?;
    let response = json!({"ok": true, "path": rel});
    store_share_receipt(&state, idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_dir_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumJson(request): AxumJson<WorkspaceShareDirRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    if let Some(receipt) = replay_share_receipt(&state, request.idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let rel = sanitize_upload_rel(request.path.as_str())?;
    let target = resolve_upload_target(&root, rel.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share path already exists",
        ));
    }
    create_target_parent_within_root(&root, &target)?;
    fs::create_dir(&target)
        .map_err(|error| ApiError::msg(format!("create workspace share directory: {error}")))?;
    append_share_audit(&state, principal, "mkdir", rel.as_str())?;
    let response = json!({"ok": true, "path": rel, "isDir": true});
    store_share_receipt(&state, request.idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_rename_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumJson(request): AxumJson<WorkspaceShareRenameRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Organize)?;
    if let Some(receipt) = replay_share_receipt(&state, request.idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let from_rel = sanitize_upload_rel(request.from_path.as_str())?;
    let to_rel = build_rename_target_rel(from_rel.as_str(), request.to_path.as_str())?;
    let source = resolve_existing_upload_file(&root, from_rel.as_str())?;
    let source_metadata = fs::metadata(&source)
        .map_err(|error| ApiError::msg(format!("stat workspace share source: {error}")))?;
    require_expected_revision(
        from_rel.as_str(),
        &source_metadata,
        request.expected_revision.as_str(),
    )?;
    let target = resolve_upload_target(&root, to_rel.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share target already exists",
        ));
    }
    create_target_parent_within_root(&root, &target)?;
    fs::rename(&source, &target)
        .map_err(|error| ApiError::msg(format!("rename workspace share entry: {error}")))?;
    append_share_audit(
        &state,
        principal,
        "rename",
        format!("{from_rel} -> {to_rel}").as_str(),
    )?;
    let target_metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("stat renamed workspace share entry: {error}")))?;
    let response = json!({
        "ok": true,
        "oldPath": from_rel,
        "newPath": to_rel,
        "revision": entry_revision(to_rel.as_str(), &target_metadata),
    });
    store_share_receipt(&state, request.idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_move_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumJson(request): AxumJson<WorkspaceShareMoveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Organize)?;
    if let Some(receipt) = replay_share_receipt(&state, request.idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let from_rel = sanitize_upload_rel(request.from_path.as_str())?;
    let target_rel = build_move_target_rel(from_rel.as_str(), request.to_dir.as_deref())?;
    if target_rel.starts_with(&format!("{from_rel}/")) {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "cannot move a workspace share directory into itself",
        ));
    }
    let source = resolve_existing_upload_file(&root, from_rel.as_str())?;
    let source_metadata = fs::metadata(&source)
        .map_err(|error| ApiError::msg(format!("stat workspace share source: {error}")))?;
    require_expected_revision(
        from_rel.as_str(),
        &source_metadata,
        request.expected_revision.as_str(),
    )?;
    let target = resolve_upload_target(&root, target_rel.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share target already exists",
        ));
    }
    create_target_parent_within_root(&root, &target)?;
    fs::rename(&source, &target)
        .map_err(|error| ApiError::msg(format!("move workspace share entry: {error}")))?;
    append_share_audit(
        &state,
        principal,
        "move",
        format!("{from_rel} -> {target_rel}").as_str(),
    )?;
    let target_metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("stat moved workspace share entry: {error}")))?;
    let revision = entry_revision(target_rel.as_str(), &target_metadata);
    let response = json!({
        "ok": true,
        "oldPath": from_rel,
        "newPath": target_rel,
        "revision": revision,
    });
    store_share_receipt(&state, request.idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_delete(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<WorkspaceShareDeleteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Delete)?;
    if let Some(receipt) = replay_share_receipt(&state, query.idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let rel = sanitize_upload_rel(query.path.as_str())?;
    let target = resolve_existing_upload_file(&root, rel.as_str())?;
    let metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("stat workspace share delete target: {error}")))?;
    require_expected_revision(rel.as_str(), &metadata, query.expected_revision.as_str())?;
    if target.is_dir() {
        if fs::read_dir(&target)
            .map_err(|error| ApiError::msg(format!("read workspace share directory: {error}")))?
            .next()
            .is_some()
        {
            return Err(ApiError::status(
                StatusCode::CONFLICT,
                "workspace share directory is not empty",
            ));
        }
        fs::remove_dir(&target)
            .map_err(|error| ApiError::msg(format!("delete workspace share directory: {error}")))?;
    } else {
        fs::remove_file(&target)
            .map_err(|error| ApiError::msg(format!("delete workspace share file: {error}")))?;
    }
    append_share_audit(&state, principal, "delete", rel.as_str())?;
    let response = json!({"ok": true, "oldPath": rel});
    store_share_receipt(&state, query.idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_download_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<WorkspaceShareDownloadQuery>,
) -> Result<Response, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::View)?;
    let root = ensure_share_root(&state)?;
    let rel = sanitize_upload_rel(query.path.as_str())?;
    let target = resolve_existing_upload_file(&root, rel.as_str())?;
    if !target.is_file() {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "cannot download a workspace share directory",
        ));
    }
    let metadata = fs::metadata(&target)
        .map_err(|error| ApiError::msg(format!("stat workspace share file: {error}")))?;
    if let Some(expected) = query.expected_revision.as_deref() {
        require_expected_revision(rel.as_str(), &metadata, expected)?;
    }
    let file_name = file_name_from_upload_rel(rel.as_str())?;
    let file_len = metadata.len();
    let content_type = download_content_type(&target);
    let stream_path = target.clone();
    let stream = async_stream::stream! {
        let mut file = match fs::File::open(&stream_path) {
            Ok(file) => file,
            Err(error) => {
                yield Err(std::io::Error::other(error.to_string()));
                return;
            }
        };
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            let read = match file.read(&mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    yield Err(std::io::Error::other(error.to_string()));
                    return;
                }
            };
            if read == 0 {
                break;
            }
            yield Ok(Bytes::from(buffer[..read].to_vec()));
        }
    };
    append_share_audit(&state, principal, "download", rel.as_str())?;
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(file_len.to_string().as_str())
            .map_err(|error| ApiError::msg(format!("invalid workspace share length: {error}")))?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        content_disposition_attachment(file_name.as_str())?,
    );
    Ok(response)
}

pub async fn workspace_share_chunk_init_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumJson(request): AxumJson<WorkspaceShareChunkInitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    if let Some(receipt) = replay_share_receipt(&state, request.idempotency_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let chunk_size = normalize_chunk_size(request.chunk_size);
    let total_chunks = total_chunk_count(request.size_bytes, chunk_size)?;
    let rel_path = build_upload_rel(request.dir.as_deref(), request.file_name.as_str())?;
    let target = resolve_upload_target(&root, rel_path.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share target already exists",
        ));
    }
    let upload_id = stable_upload_id(
        rel_path.as_str(),
        request.size_bytes,
        chunk_size,
        request.last_modified_ms,
    );
    let session_dir = upload_chunk_session_dir(&root, upload_id.as_str())?;
    let meta = UploadChunkSessionMeta {
        upload_id: upload_id.clone(),
        rel_path: rel_path.clone(),
        file_name: request.file_name,
        size_bytes: request.size_bytes,
        chunk_size,
        total_chunks,
        last_modified_ms: request.last_modified_ms,
    };
    fs::create_dir_all(&session_dir).map_err(|error| {
        ApiError::msg(format!("create workspace share upload session: {error}"))
    })?;
    write_chunk_session_meta(&session_dir, &meta)?;
    let uploaded_chunks = list_uploaded_chunk_indexes(&session_dir)?;
    let response = json!({
        "ok": true,
        "uploadId": upload_id,
        "path": rel_path,
        "chunkSize": chunk_size,
        "totalChunks": total_chunks,
        "uploadedChunks": uploaded_chunks,
    });
    store_share_receipt(&state, request.idempotency_key.as_str(), &response)?;
    Ok(Json(response))
}

pub async fn workspace_share_chunk_status_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<UploadChunkStatusQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    let root = ensure_share_root(&state)?;
    let session_dir = upload_chunk_session_dir(&root, query.upload_id.as_str())?;
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

pub async fn workspace_share_chunk_put(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<UploadChunkPutQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    let root = ensure_share_root(&state)?;
    let session_dir = upload_chunk_session_dir(&root, query.upload_id.as_str())?;
    let meta = read_chunk_session_meta(&session_dir)?;
    if query.index >= meta.total_chunks || body.len() != expected_chunk_len(&meta, query.index) {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "invalid workspace share upload chunk",
        ));
    }
    fs::write(
        upload_chunk_part_path(&session_dir, query.index),
        body.as_ref(),
    )
    .map_err(|error| ApiError::msg(format!("write workspace share upload chunk: {error}")))?;
    Ok(Json(json!({
        "ok": true,
        "uploadId": meta.upload_id,
        "index": query.index,
        "receivedBytes": body.len(),
    })))
}

pub async fn workspace_share_chunk_complete_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumJson(request): AxumJson<UploadChunkCompleteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = principal.as_ref().map(|value| &value.0);
    ensure_share_permission(principal, SharePermission::Upload)?;
    let completion_key = format!("complete-{}", request.upload_id);
    if let Some(receipt) = replay_share_receipt(&state, completion_key.as_str())? {
        return Ok(Json(receipt));
    }
    let root = ensure_share_root(&state)?;
    let session_dir = upload_chunk_session_dir(&root, request.upload_id.as_str())?;
    let meta = read_chunk_session_meta(&session_dir)?;
    let target = resolve_upload_target(&root, meta.rel_path.as_str())?;
    if target.exists() {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "workspace share target already exists",
        ));
    }
    create_target_parent_within_root(&root, &target)?;
    let temp = session_dir.join("assembled.tmp");
    let file = fs::File::create(&temp)
        .map_err(|error| ApiError::msg(format!("create workspace share upload temp: {error}")))?;
    let mut writer = BufWriter::new(file);
    let mut written = 0u64;
    for index in 0..meta.total_chunks {
        let chunk = fs::read(upload_chunk_part_path(&session_dir, index)).map_err(|error| {
            ApiError::msg(format!("read workspace share upload chunk: {error}"))
        })?;
        if chunk.len() != expected_chunk_len(&meta, index) {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "workspace share upload chunk is incomplete",
            ));
        }
        writer
            .write_all(&chunk)
            .map_err(|error| ApiError::msg(format!("assemble workspace share upload: {error}")))?;
        written += chunk.len() as u64;
    }
    writer
        .flush()
        .map_err(|error| ApiError::msg(format!("flush workspace share upload: {error}")))?;
    if written != meta.size_bytes {
        let _ = fs::remove_file(&temp);
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "workspace share upload size mismatch",
        ));
    }
    fs::rename(&temp, &target)
        .map_err(|error| ApiError::msg(format!("finalize workspace share upload: {error}")))?;
    append_share_audit(&state, principal, "upload", meta.rel_path.as_str())?;
    let response = json!({
        "ok": true,
        "path": meta.rel_path,
        "sizeBytes": meta.size_bytes,
    });
    store_share_receipt(&state, completion_key.as_str(), &response)?;
    let _ = fs::remove_dir_all(&session_dir);
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_listing_treats_directories_as_categories_and_files_as_resources() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        fs::create_dir_all(root.join("reports")).expect("create category");
        fs::write(root.join("reports").join("weekly.pdf"), b"pdf").expect("write file");
        fs::write(root.join("readme.txt"), b"hello").expect("write root file");

        let entries = list_share_entries("", root, None).expect("list root");
        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].path, "reports");
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].path, "readme.txt");

        let directories = collect_share_directories(root).expect("collect tree");
        assert_eq!(directories, vec!["reports"]);
    }

    #[test]
    fn share_directory_resolution_rejects_parent_escape() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert!(resolve_share_dir(temp.path(), Some("../outside")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn share_target_parent_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let share = tempfile::tempdir().expect("share dir");
        let outside = tempfile::tempdir().expect("outside dir");
        symlink(outside.path(), share.path().join("escape")).expect("create symlink");
        let target = share.path().join("escape").join("file.txt");
        assert!(create_target_parent_within_root(share.path(), &target).is_err());
        assert!(!outside.path().join("file.txt").exists());
    }
}
