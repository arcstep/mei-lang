use std::path::{Component, Path};

/// RFC3339 UTC timestamp for CLI / prebuild logs.
pub fn log_timestamp_rfc3339() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

/// Human-readable byte size (binary units).
pub fn format_bytes_human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Prefer a workspace-relative path for logs; fall back to a normalized relative path.
pub fn path_for_log(workspace: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(workspace) {
        return normalize_log_path(relative);
    }
    if let (Ok(workspace), Ok(path)) = (workspace.canonicalize(), path.canonicalize()) {
        if let Ok(relative) = path.strip_prefix(&workspace) {
            return normalize_log_path(relative);
        }
    }
    normalize_log_path(path)
}

fn normalize_log_path(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::RootDir | Component::Prefix(_) => {
                parts.push(component.as_os_str().to_string_lossy().into_owned())
            }
        }
    }
    parts.join("/")
}

/// Sum on-disk bytes for all regular files under `root`.
pub fn dir_tree_bytes(root: &Path) -> u64 {
    if !root.is_dir() {
        return 0;
    }
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn path_for_log_strips_workspace_prefix() {
        let workspace = PathBuf::from("/tmp/ws-demo-v2");
        let path = workspace.join("apps/data-demo/env/2.0.7/build/exchange/data-demo.meibundle");
        assert_eq!(
            path_for_log(workspace.as_path(), path.as_path()),
            "apps/data-demo/env/2.0.7/build/exchange/data-demo.meibundle"
        );
    }

    #[test]
    fn format_bytes_human_uses_binary_units() {
        assert_eq!(format_bytes_human(0), "0 B");
        assert_eq!(format_bytes_human(1536), "1.5 KiB");
    }
}
