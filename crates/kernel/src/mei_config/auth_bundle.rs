use std::path::{Path, PathBuf};

use anyhow::Result;

use super::io::write_workspace_config;
use super::types::{MeiConfig, WorkspaceAuthConfig, WorkspaceConfig, MEI_CONFIG_FILENAME};
use super::workspace_paths::workspace_config_path;

/// 工作区 segment 根目录的 `.mei-workspace.json`。
pub fn workspace_auth_config_path(segment_root: &Path) -> PathBuf {
    workspace_config_path(segment_root)
}

#[derive(Debug, Clone)]
pub struct WorkspaceAuthBundle {
    pub auth: WorkspaceAuthConfig,
    pub config_path: PathBuf,
}

fn workspace_auth_section_empty(auth: &WorkspaceAuthConfig) -> bool {
    auth.users.is_empty()
        && auth
            .jwt_secret
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && auth.key_pair.public_key_pem.trim().is_empty()
        && auth.key_pair.private_key_pem.trim().is_empty()
}

/// 读取工作区认证配置：优先 `{segment_root}/.mei-workspace.json#auth`，
/// 如为空则兼容回退到同级误写入的 `.mei-config.json#auth`。
pub fn load_workspace_auth_bundle(segment_root: &Path) -> WorkspaceAuthBundle {
    let config_path = workspace_auth_config_path(segment_root);
    let mut auth = if config_path.is_file() {
        WorkspaceConfig::load_or_default(&config_path).auth
    } else {
        WorkspaceAuthConfig::default()
    };
    if workspace_auth_section_empty(&auth) {
        let misplaced_path = segment_root.join(MEI_CONFIG_FILENAME);
        if misplaced_path.is_file() {
            let misplaced_auth = MeiConfig::load_or_default(&misplaced_path).auth;
            if !workspace_auth_section_empty(&misplaced_auth) {
                auth = misplaced_auth;
            }
        }
    }
    WorkspaceAuthBundle { auth, config_path }
}

/// 将认证段写入工作区根 `.mei-workspace.json`。
pub fn write_workspace_auth_bundle(
    segment_root: &Path,
    auth: &WorkspaceAuthConfig,
) -> Result<PathBuf> {
    let path = workspace_auth_config_path(segment_root);
    let mut config = if path.is_file() {
        WorkspaceConfig::load_or_default(&path)
    } else {
        WorkspaceConfig::default()
    };
    if config.schema_version == 0 {
        config.schema_version = 1;
    }
    config.auth = auth.clone();
    write_workspace_config(&path, &config)?;
    Ok(path)
}
