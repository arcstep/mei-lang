use std::{
    hash::{Hash, Hasher},
    path::Path as FsPath,
};

use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub(super) struct MeiFileEntry {
    pub(super) relative_path: String,
    pub(super) modified_epoch_ms: u128,
}

pub(super) fn collect_mei_file_entries(app_root: &FsPath) -> Vec<MeiFileEntry> {
    let mut files = WalkDir::new(app_root)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei"))
        .filter_map(|entry| {
            let relative_path = entry
                .path()
                .strip_prefix(app_root)
                .ok()
                .and_then(|path| path.to_str())
                .map(|path| path.replace('\\', "/"))?;
            let modified_epoch_ms = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_millis())
                .unwrap_or(0);
            Some(MeiFileEntry {
                relative_path,
                modified_epoch_ms,
            })
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

pub(super) fn build_mei_files_revision(entries: &[MeiFileEntry]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for entry in entries {
        entry.relative_path.hash(&mut hasher);
        entry.modified_epoch_ms.hash(&mut hasher);
    }
    hasher.finish()
}
