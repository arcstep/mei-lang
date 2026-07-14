use std::path::{Path, PathBuf};

use mei_lang_kernel::resolve_app_root;

/// Phase 8.5: single launch file at `{workspace}/apps/{app}/launch.json`.
pub fn launch_json_path(workspace: &Path, app_id: &str) -> PathBuf {
    resolve_app_root(workspace, app_id).join("launch.json")
}

/// Legacy directory `{workspace}/apps/{app}/launch/` (migration / cleanup only).
pub fn app_launch_dir(workspace: &Path, app_id: &str) -> PathBuf {
    resolve_app_root(workspace, app_id).join("launch")
}

/// Compatibility alias — always the single `launch.json`.
pub fn default_launch_path(workspace: &Path, app_id: &str) -> PathBuf {
    launch_json_path(workspace, app_id)
}

/// Compatibility alias — always the single `launch.json` (name ignored).
pub fn launch_config_path(workspace: &Path, app_id: &str, _name: &str) -> PathBuf {
    launch_json_path(workspace, app_id)
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

/// Ensure the app root exists (launch.json parent).
pub fn ensure_app_launch_dir(workspace: &Path, app_id: &str) -> std::io::Result<PathBuf> {
    let app_root = resolve_app_root(workspace, app_id);
    std::fs::create_dir_all(&app_root)?;
    Ok(app_root)
}

/// Resolve launch path: absolute/relative `.json` path, or the single `launch.json`.
pub fn resolve_launch_path(workspace: &Path, app_id: &str, config: &str) -> PathBuf {
    let trimmed = config.trim();
    if trimmed.is_empty() || trimmed == "default" || trimmed == "launch" {
        return launch_json_path(workspace, app_id);
    }
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
        // Named configs under launch/ are retired; always bind the single file.
        launch_json_path(workspace, app_id)
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
    fn launch_json_nests_under_app_root() {
        let ws = PathBuf::from("/tmp/ws");
        assert_eq!(
            launch_json_path(&ws, "mini-data"),
            PathBuf::from("/tmp/ws/apps/mini-data/launch.json")
        );
        assert_eq!(
            resolve_launch_path(&ws, "mini-data", "default"),
            PathBuf::from("/tmp/ws/apps/mini-data/launch.json")
        );
        assert_eq!(
            resolve_launch_path(&ws, "mini-data", "scoped-rail"),
            PathBuf::from("/tmp/ws/apps/mini-data/launch.json")
        );
        assert_eq!(
            app_runtime_root(&ws, "mini-data"),
            PathBuf::from("/tmp/ws/deploy/runtime/apps/mini-data")
        );
    }
}
