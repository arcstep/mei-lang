use std::path::{Path, PathBuf};

pub(crate) fn resolve_source_path(app_root: &Path, source_path: &str) -> PathBuf {
    let path = Path::new(source_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        app_root.join(path)
    }
}

pub(crate) fn resolve_db_path(app_root: &Path, dsn: &str) -> PathBuf {
    let raw = dsn
        .strip_prefix("sqlite://")
        .map(ToString::to_string)
        .unwrap_or_else(|| dsn.to_string());
    resolve_source_path(app_root, raw.as_str())
}
