use std::path::{Path, PathBuf};

pub(crate) fn resolve_source_path(app_root: &Path, source_path: &str) -> PathBuf {
    mei_lang_kernel::resolve_versioned_source_path(app_root, source_path)
}

pub(crate) fn resolve_db_path(app_root: &Path, dsn: &str) -> PathBuf {
    let raw = dsn
        .strip_prefix("sqlite://")
        .map(ToString::to_string)
        .unwrap_or_else(|| dsn.to_string());
    resolve_source_path(app_root, raw.as_str())
}
