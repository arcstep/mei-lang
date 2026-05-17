use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use walkdir::WalkDir;

use crate::AppState;

pub(crate) fn compile_revision(state: &AppState, app_id: &str, components_root: &Path) -> u128 {
    let app_root = state.source_root.join(app_id);
    let app_mtime = directory_latest_modified_ms(&app_root).unwrap_or(0);
    let components_mtime = directory_latest_modified_ms(components_root).unwrap_or(0);
    app_mtime.max(components_mtime)
}

fn directory_latest_modified_ms(path: &Path) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path).into_iter().flatten() {
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

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}
