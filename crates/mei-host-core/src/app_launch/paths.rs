use std::path::{Path, PathBuf};

use mei_lang_kernel::resolve_app_root;

/// `{workspace}/apps/{app}/launch/`
pub fn app_launch_dir(workspace: &Path, app_id: &str) -> PathBuf {
    resolve_app_root(workspace, app_id).join("launch")
}

/// `{workspace}/apps/{app}/launch/{name}.json`
pub fn launch_config_path(workspace: &Path, app_id: &str, name: &str) -> PathBuf {
    app_launch_dir(workspace, app_id).join(format!("{}.json", sanitize_name(name)))
}

/// `{workspace}/apps/{app}/launch/default.json`
pub fn default_launch_path(workspace: &Path, app_id: &str) -> PathBuf {
    launch_config_path(workspace, app_id, "default")
}

/// Ephemeral runtime root for a single app (not multi-instance).
/// `{workspace}/deploy/runtime/apps/{app_id}/`
pub fn app_runtime_root(workspace: &Path, app_id: &str) -> PathBuf {
    workspace
        .join("deploy")
        .join("runtime")
        .join("apps")
        .join(sanitize_name(app_id))
}

pub fn ensure_app_launch_dir(workspace: &Path, app_id: &str) -> std::io::Result<PathBuf> {
    let dir = app_launch_dir(workspace, app_id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Resolve a launch path from either a workspace-relative file or a launch name.
pub fn resolve_launch_path(workspace: &Path, app_id: &str, config: &str) -> PathBuf {
    let trimmed = config.trim();
    let as_path = Path::new(trimmed);
    if as_path.extension().and_then(|e| e.to_str()) == Some("json")
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        if as_path.is_absolute() {
            as_path.to_path_buf()
        } else {
            workspace.join(as_path)
        }
    } else {
        launch_config_path(workspace, app_id, trimmed)
    }
}

fn sanitize_name(name: &str) -> &str {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "_invalid"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn launch_paths_nest_under_app() {
        let ws = PathBuf::from("/tmp/ws");
        assert_eq!(
            app_launch_dir(&ws, "mini-data"),
            PathBuf::from("/tmp/ws/apps/mini-data/launch")
        );
        assert_eq!(
            launch_config_path(&ws, "mini-data", "scoped-rail"),
            PathBuf::from("/tmp/ws/apps/mini-data/launch/scoped-rail.json")
        );
        assert_eq!(
            app_runtime_root(&ws, "mini-data"),
            PathBuf::from("/tmp/ws/deploy/runtime/apps/mini-data")
        );
    }
}
