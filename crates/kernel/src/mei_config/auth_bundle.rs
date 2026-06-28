use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::io::{write_string_atomically, write_workspace_config};
use super::types::{
    MeiConfig, WorkspaceAuthConfig, WorkspaceConfig, WorkspaceHostState, DEFAULT_HOST_STATE_ID,
    MEI_CONFIG_FILENAME, WORKSPACE_HOST_STATE_SCHEMA_VERSION,
};
use super::workspace_paths::{resolve_workspace_hosts_root, workspace_config_path};

fn normalize_host_state_id(raw: Option<&str>) -> String {
    let trimmed = raw.unwrap_or("").trim();
    if trimmed.is_empty() {
        return DEFAULT_HOST_STATE_ID.to_string();
    }
    let mut normalized = String::new();
    let mut previous_dash = false;
    for ch in trimmed.chars() {
        let keep = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.');
        if keep {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        DEFAULT_HOST_STATE_ID.to_string()
    } else {
        normalized
    }
}

pub fn workspace_auth_host_id(segment_root: &Path) -> String {
    let config_path = workspace_config_path(segment_root);
    if config_path.is_file() {
        let config = WorkspaceConfig::load_or_default(&config_path);
        return normalize_host_state_id(config.workspace.deploy_host.as_deref());
    }
    DEFAULT_HOST_STATE_ID.to_string()
}

pub fn workspace_auth_state_dir(segment_root: &Path) -> PathBuf {
    resolve_workspace_hosts_root(segment_root)
}

/// 工作区 segment 根目录的 host-state auth 文件。
pub fn workspace_auth_config_path(segment_root: &Path) -> PathBuf {
    workspace_auth_state_dir(segment_root).join(format!(
        "{}.state.json",
        workspace_auth_host_id(segment_root)
    ))
}

#[derive(Debug, Clone)]
pub struct WorkspaceAuthBundle {
    pub auth: WorkspaceAuthConfig,
    pub workspace_config_path: PathBuf,
    pub config_path: PathBuf,
    pub loaded_from: String,
    pub loaded_from_path: Option<PathBuf>,
}

fn workspace_auth_section_empty(auth: &WorkspaceAuthConfig) -> bool {
    auth.is_empty()
}

fn bundle_with_source(
    auth: WorkspaceAuthConfig,
    workspace_config_path: PathBuf,
    state_path: PathBuf,
    loaded_from: &str,
    loaded_from_path: Option<PathBuf>,
) -> WorkspaceAuthBundle {
    WorkspaceAuthBundle {
        auth,
        workspace_config_path,
        config_path: state_path,
        loaded_from: loaded_from.to_string(),
        loaded_from_path,
    }
}

/// 读取工作区认证配置：优先新 host-state，其次 `.mei-workspace.json#auth`，
/// 最后兼容回退到同级误写入的 `.mei-config.json#auth`。
pub fn load_workspace_auth_bundle(segment_root: &Path) -> WorkspaceAuthBundle {
    let workspace_path = workspace_config_path(segment_root);
    let state_path = workspace_auth_config_path(segment_root);
    let mut candidate_paths = vec![state_path.clone()];
    let default_state_path =
        workspace_auth_state_dir(segment_root).join(format!("{DEFAULT_HOST_STATE_ID}.state.json"));
    if default_state_path != state_path {
        candidate_paths.push(default_state_path);
    }
    for candidate in candidate_paths {
        if candidate.is_file() {
            let state = WorkspaceHostState::load_or_default(&candidate);
            if !workspace_auth_section_empty(&state.auth) {
                return bundle_with_source(
                    state.auth,
                    workspace_path,
                    state_path.clone(),
                    "workspace_host_state",
                    Some(candidate),
                );
            }
        }
    }
    let auth = if workspace_path.is_file() {
        WorkspaceConfig::load_or_default(&workspace_path).auth
    } else {
        WorkspaceAuthConfig::default()
    };
    if workspace_auth_section_empty(&auth) {
        let misplaced_path = segment_root.join(MEI_CONFIG_FILENAME);
        if misplaced_path.is_file() {
            let misplaced_auth = MeiConfig::load_or_default(&misplaced_path).auth;
            if !workspace_auth_section_empty(&misplaced_auth) {
                return bundle_with_source(
                    misplaced_auth,
                    workspace_path,
                    state_path,
                    "legacy_mei_config_auth",
                    Some(misplaced_path),
                );
            }
        }
        return bundle_with_source(auth, workspace_path, state_path, "default", None);
    }
    bundle_with_source(
        auth,
        workspace_path.clone(),
        state_path.clone(),
        "workspace_config_auth",
        Some(workspace_path),
    )
}

fn scrub_workspace_auth_section(segment_root: &Path) -> Result<()> {
    let path = workspace_config_path(segment_root);
    if !path.is_file() {
        return Ok(());
    }
    let mut config = WorkspaceConfig::load_or_default(&path);
    if workspace_auth_section_empty(&config.auth) {
        return Ok(());
    }
    config.auth = WorkspaceAuthConfig::default();
    write_workspace_config(&path, &config)
}

/// 将认证段写入工作区 host-state 文件，并从 `.mei-workspace.json` 中剥离旧 auth。
pub fn write_workspace_auth_bundle(
    segment_root: &Path,
    auth: &WorkspaceAuthConfig,
) -> Result<PathBuf> {
    let path = workspace_auth_config_path(segment_root);
    let mut state = if path.is_file() {
        WorkspaceHostState::load_or_default(&path)
    } else {
        WorkspaceHostState::default()
    };
    if state.schema_version == 0 {
        state.schema_version = WORKSPACE_HOST_STATE_SCHEMA_VERSION;
    }
    state.host_id = Some(workspace_auth_host_id(segment_root));
    state.auth = auth.clone();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(&state)?;
    write_string_atomically(&path, raw.as_str())?;
    scrub_workspace_auth_section(segment_root)?;
    Ok(path)
}
