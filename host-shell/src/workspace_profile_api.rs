use axum::{
    body::Bytes,
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_lang_kernel::{WorkspaceProfileError, WorkspaceProfileService};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path as FsPath, PathBuf};

use crate::state::SharedState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRuntimeProfile {
    pub id: String,
    pub file: String,
    pub revision: String,
    pub source: String,
    #[serde(skip_serializing)]
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutWorkspaceProfileRequest {
    pub expected_revision: Option<String>,
    pub config: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceProfilePreviewRequest {
    pub config: Value,
}

pub async fn workspace_profiles_get(
    State(state): State<SharedState>,
) -> Result<Json<Value>, WorkspaceProfileApiError> {
    let service = service(&state);
    let profiles = service.list()?;
    Ok(Json(json!({ "profiles": profiles })))
}

pub async fn runtime_profile_get(State(state): State<SharedState>) -> Json<Value> {
    let guard = state.read().expect("state lock");
    let active = read_host_control_state(guard.ctx.workspace_root.as_path())
        .and_then(|value| value.get("activeProfile").cloned());
    let job_running = guard
        .ops_job
        .as_ref()
        .is_some_and(crate::build_ops::OpsJobState::is_running);
    let status = control_status(&guard, active.is_some(), job_running);
    let access_state = access_status(&guard, active.is_some());
    Json(json!({
        "status": status,
        "selectedProfile": guard.selected_profile_id.as_ref().map(|id| json!({
            "id": id,
            "file": guard.selected_profile_file,
            "revision": guard.selected_profile_revision,
            "source": guard.selected_profile_source,
        })),
        "activeProfile": active,
        "dataPlane": {
            "enabled": guard.data_plane_enabled,
            "defaultAppId": guard.default_app(),
        },
        "access": {
            "state": access_state,
            "enabled": access_state == "ready",
        },
        "startup": {
            "phase": guard.startup_phase,
            "detail": guard.startup_detail,
            "error": guard.startup_error,
        },
    }))
}

fn control_status(
    state: &crate::state::ShellState,
    has_active_profile: bool,
    job_running: bool,
) -> &'static str {
    if job_running {
        "building"
    } else if state.startup_error.is_some() {
        "degraded"
    } else if state.data_plane_enabled && state.imported {
        "ready"
    } else if !has_active_profile {
        "unconfigured"
    } else {
        "degraded"
    }
}

fn access_status(state: &crate::state::ShellState, has_active_profile: bool) -> &'static str {
    if state.data_plane_enabled && state.imported {
        "ready"
    } else if !has_active_profile {
        "unconfigured"
    } else {
        "disabled"
    }
}

pub fn resolve_runtime_profile(
    workspace: &FsPath,
    explicit_path: Option<&FsPath>,
) -> anyhow::Result<Option<ResolvedRuntimeProfile>> {
    let service = WorkspaceProfileService::new(workspace);
    if let Some(explicit) = explicit_path {
        let path = resolve_explicit_profile_path(workspace, explicit)?;
        let id = profile_id_for_path(workspace, path.as_path())?;
        let document = service
            .read(id.as_str())
            .map_err(|error| anyhow::anyhow!("workspace config {}: {error}", path.display()))?;
        return Ok(Some(ResolvedRuntimeProfile {
            id: document.id,
            file: document.file,
            revision: document.revision,
            source: "cli".to_string(),
            path,
        }));
    }

    if let Some(profile_id) = last_successful_profile_id(workspace) {
        if let Ok(document) = service.read(profile_id.as_str()) {
            return Ok(Some(ResolvedRuntimeProfile {
                id: document.id,
                file: document.file.clone(),
                revision: document.revision,
                source: "last_successful".to_string(),
                path: workspace.join(document.file),
            }));
        }
    }

    match service.read("default") {
        Ok(document) => Ok(Some(ResolvedRuntimeProfile {
            id: document.id,
            file: document.file.clone(),
            revision: document.revision,
            source: "workspace_default".to_string(),
            path: workspace.join(document.file),
        })),
        Err(WorkspaceProfileError::NotFound) => Ok(None),
        Err(error) => Err(anyhow::anyhow!("workspace.json profile: {error}")),
    }
}

pub fn install_selected_profile(state: &SharedState, profile: Option<&ResolvedRuntimeProfile>) {
    let mut guard = state.write().expect("state lock");
    guard.selected_profile_id = profile.map(|entry| entry.id.clone());
    guard.selected_profile_file = profile.map(|entry| entry.file.clone());
    guard.selected_profile_revision = profile.map(|entry| entry.revision.clone());
    guard.selected_profile_source = profile.map(|entry| entry.source.clone());
}

pub fn last_successful_profile_id(workspace: &FsPath) -> Option<String> {
    read_host_control_state(workspace)?
        .get("lastSuccessfulApply")
        .and_then(|value| value.get("profileId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub fn read_host_control_state(workspace: &FsPath) -> Option<Value> {
    let state = mei_host_core::read_host_control_state(workspace)?;
    serde_json::to_value(state).ok()
}

fn resolve_explicit_profile_path(
    workspace: &FsPath,
    requested: &FsPath,
) -> anyhow::Result<PathBuf> {
    let raw = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        workspace.join(requested)
    };
    let candidates = if raw.is_file() {
        vec![raw]
    } else {
        let name = requested.to_string_lossy();
        vec![
            workspace.join("configs").join(name.as_ref()),
            workspace
                .join("configs")
                .join(format!("{}.json", name.trim_end_matches(".json"))),
        ]
    };
    let candidate = candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| anyhow::anyhow!("workspace config not found: {}", requested.display()))?;
    let root = fs::canonicalize(workspace)?;
    let path = fs::canonicalize(candidate)?;
    if !path.starts_with(&root) {
        anyhow::bail!("workspace config must stay inside {}", root.display());
    }
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| anyhow::anyhow!("workspace config path is outside workspace"))?;
    let allowed = relative == FsPath::new("workspace.json")
        || (relative.parent() == Some(FsPath::new("configs"))
            && relative.extension().and_then(|value| value.to_str()) == Some("json"));
    if !allowed {
        anyhow::bail!("workspace config must be workspace.json or configs/*.json");
    }
    Ok(path)
}

fn profile_id_for_path(workspace: &FsPath, path: &FsPath) -> anyhow::Result<String> {
    let root = fs::canonicalize(workspace)?;
    let relative = path.strip_prefix(root)?;
    if relative == FsPath::new("workspace.json") {
        return Ok("default".to_string());
    }
    relative
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("workspace config filename is invalid"))
}

pub async fn workspace_profile_get(
    Path(id): Path<String>,
    State(state): State<SharedState>,
) -> Result<Json<Value>, WorkspaceProfileApiError> {
    let profile = service(&state).read(&id)?;
    Ok(Json(
        serde_json::to_value(profile).expect("workspace profile document serialization"),
    ))
}

pub async fn workspace_profile_put(
    Path(id): Path<String>,
    State(state): State<SharedState>,
    body: Result<Json<PutWorkspaceProfileRequest>, JsonRejection>,
) -> Result<Json<Value>, WorkspaceProfileApiError> {
    let Json(request) = body.map_err(|_| WorkspaceProfileApiError::InvalidRequest)?;
    let profile = service(&state)
        .save(&id, request.expected_revision.as_deref(), request.config)
        .map_err(WorkspaceProfileApiError::Profile)?;
    Ok(Json(
        serde_json::to_value(profile).expect("workspace profile document serialization"),
    ))
}

pub async fn workspace_profile_validate_post(
    Path(id): Path<String>,
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<Json<Value>, WorkspaceProfileApiError> {
    let service = service(&state);
    let preview = optional_preview_config(body)?;
    let document = service.read(&id)?;
    let validation = preview
        .as_ref()
        .map(|config| service.validate_config(config))
        .unwrap_or_else(|| document.validation.clone());
    Ok(Json(json!({
        "id": document.id,
        "revision": document.revision,
        "validation": validation
    })))
}

pub async fn workspace_profile_dry_run_post(
    Path(id): Path<String>,
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<Json<Value>, WorkspaceProfileApiError> {
    let service = service(&state);
    let dry_run = match optional_preview_config(body)? {
        Some(config) => service.dry_run_config(&id, config)?,
        None => service.dry_run(&id)?,
    };
    Ok(Json(
        serde_json::to_value(dry_run).expect("workspace profile dry-run serialization"),
    ))
}

fn optional_preview_config(body: Bytes) -> Result<Option<Value>, WorkspaceProfileApiError> {
    if body.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice::<WorkspaceProfilePreviewRequest>(&body)
        .map(|request| Some(request.config))
        .map_err(|_| WorkspaceProfileApiError::InvalidRequest)
}

fn service(state: &SharedState) -> WorkspaceProfileService {
    let workspace_root = state.read().expect("state lock").ctx.workspace_root.clone();
    WorkspaceProfileService::new(workspace_root)
}

#[derive(Debug)]
pub enum WorkspaceProfileApiError {
    Profile(WorkspaceProfileError),
    InvalidRequest,
}

impl From<WorkspaceProfileError> for WorkspaceProfileApiError {
    fn from(error: WorkspaceProfileError) -> Self {
        Self::Profile(error)
    }
}

impl IntoResponse for WorkspaceProfileApiError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match self {
            WorkspaceProfileApiError::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "workspace profile request body is invalid",
                None,
            ),
            WorkspaceProfileApiError::Profile(error) => match error {
                WorkspaceProfileError::InvalidId => (
                    StatusCode::BAD_REQUEST,
                    "invalid_profile_id",
                    "invalid workspace profile id",
                    None,
                ),
                WorkspaceProfileError::NotFound => (
                    StatusCode::NOT_FOUND,
                    "profile_not_found",
                    "workspace profile not found",
                    None,
                ),
                WorkspaceProfileError::InvalidPath => (
                    StatusCode::BAD_REQUEST,
                    "invalid_profile_path",
                    "workspace profile path is not allowed",
                    None,
                ),
                WorkspaceProfileError::InvalidJson(message) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_profile_json",
                    "workspace profile JSON is invalid",
                    Some(json!({ "parseError": message })),
                ),
                WorkspaceProfileError::InvalidSchema(issues) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_profile_schema",
                    "workspace profile schema is invalid",
                    Some(json!({ "issues": issues })),
                ),
                WorkspaceProfileError::RevisionConflict { expected, current } => (
                    StatusCode::CONFLICT,
                    "revision_conflict",
                    "workspace profile revision conflict",
                    Some(json!({
                        "expectedRevision": expected,
                        "currentRevision": current
                    })),
                ),
                WorkspaceProfileError::Io(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "profile_io_failed",
                    "workspace profile operation failed",
                    None,
                ),
            },
        };
        let mut error = json!({
            "code": code,
            "message": message
        });
        if let Some(details) = details {
            error["details"] = details;
        }
        (status, Json(json!({ "error": error }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_profile_priority_is_cli_then_last_successful_then_workspace_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("workspace.json"),
            r#"{"workspace":{"id":"default"}}"#,
        )
        .expect("default");
        fs::create_dir_all(tmp.path().join("configs")).expect("configs");
        fs::write(
            tmp.path().join("configs/recent.json"),
            r#"{"workspace":{"id":"recent"}}"#,
        )
        .expect("recent");
        fs::write(
            tmp.path().join("configs/explicit.json"),
            r#"{"workspace":{"id":"explicit"}}"#,
        )
        .expect("explicit");
        fs::create_dir_all(tmp.path().join("deploy/state")).expect("state");
        fs::write(
            tmp.path().join("deploy/state/host-control.json"),
            r#"{"lastSuccessfulApply":{"profileId":"recent"}}"#,
        )
        .expect("control");

        let recent = resolve_runtime_profile(tmp.path(), None)
            .expect("resolve recent")
            .expect("recent");
        assert_eq!(recent.id, "recent");
        assert_eq!(recent.source, "last_successful");

        let explicit =
            resolve_runtime_profile(tmp.path(), Some(FsPath::new("configs/explicit.json")))
                .expect("resolve explicit")
                .expect("explicit");
        assert_eq!(explicit.id, "explicit");
        assert_eq!(explicit.source, "cli");

        fs::remove_file(tmp.path().join("deploy/state/host-control.json")).expect("remove state");
        let default = resolve_runtime_profile(tmp.path(), None)
            .expect("resolve default")
            .expect("default");
        assert_eq!(default.id, "default");
        assert_eq!(default.source, "workspace_default");
    }

    #[test]
    fn explicit_profile_must_stay_in_workspace_config_surface() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("workspace.json"), "{}").expect("default");
        fs::write(tmp.path().join("other.json"), "{}").expect("other");
        let error = resolve_runtime_profile(tmp.path(), Some(FsPath::new("other.json")))
            .expect_err("root other.json must be rejected");
        assert!(error
            .to_string()
            .contains("workspace.json or configs/*.json"));
    }

    #[test]
    fn control_status_moves_unconfigured_building_ready_and_degraded() {
        let mut state = crate::state::ShellState::new(
            PathBuf::from("/tmp/mei-first-boot-status"),
            String::new(),
            PathBuf::from("/tmp/mei-package"),
            std::collections::BTreeMap::new(),
            false,
        );
        assert_eq!(control_status(&state, false, false), "unconfigured");
        assert_eq!(access_status(&state, false), "unconfigured");
        assert_eq!(control_status(&state, false, true), "building");

        assert_eq!(control_status(&state, true, false), "degraded");
        assert_eq!(access_status(&state, true), "disabled");
        state.set_default_app(Some("demo".to_string()));
        state.data_plane_enabled = true;
        state.imported = true;
        assert_eq!(control_status(&state, false, false), "ready");
        assert_eq!(access_status(&state, false), "ready");
        assert_eq!(control_status(&state, true, false), "ready");
        assert_eq!(access_status(&state, true), "ready");
        state.startup_error = Some("sidecar failed".to_string());
        assert_eq!(control_status(&state, true, false), "degraded");
    }
}
