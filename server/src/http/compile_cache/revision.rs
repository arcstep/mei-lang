use std::{
    env,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use walkdir::WalkDir;

use crate::AppState;

pub(crate) fn compile_revision(state: &AppState, app_id: &str, components_root: &Path) -> u128 {
    let app_root = state.source_root.join(app_id);
    if compile_revision_mode() == RevisionMode::Full {
        let app_mtime = directory_latest_full_modified_ms(&app_root).unwrap_or(0);
        let components_mtime = directory_latest_full_modified_ms(components_root).unwrap_or(0);
        return app_mtime.max(components_mtime);
    }
    let app_mtime = directory_latest_modified_ms(&app_root, RevisionScope::App).unwrap_or(0);
    let components_mtime =
        directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0);
    app_mtime.max(components_mtime)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RevisionMode {
    Relevant,
    Full,
}

#[derive(Clone, Copy)]
enum RevisionScope {
    App,
    Components,
}

fn compile_revision_mode() -> RevisionMode {
    let raw = env::var("MEI_COMPILE_REVISION_MODE").unwrap_or_default();
    if raw.trim().eq_ignore_ascii_case("full") {
        RevisionMode::Full
    } else {
        RevisionMode::Relevant
    }
}

fn directory_latest_full_modified_ms(path: &Path) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn directory_latest_modified_ms(path: &Path, scope: RevisionScope) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        if !entry.file_type().is_file() || !is_revision_relevant(entry.path(), scope) {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "node_modules" | "target" | ".mei" | "__pycache__" | "dist" | "build"
            )
        })
}

fn is_revision_relevant(path: &Path, scope: RevisionScope) -> bool {
    match scope {
        RevisionScope::App => path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mei")),
        RevisionScope::Components => is_component_manifest(path),
    }
}

fn is_component_manifest(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("component.manifest.json") {
        return false;
    }
    let normalized = normalize_path(path);
    normalized.contains("/_components/")
}

fn normalize_path(path: &Path) -> String {
    PathBuf::from(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}
