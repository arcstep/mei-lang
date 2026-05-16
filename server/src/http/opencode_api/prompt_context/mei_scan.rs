use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::Path as FsPath,
};

use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub(super) struct MeiFileEntry {
    pub(super) relative_path: String,
    pub(super) modified_epoch_ms: u128,
}

#[derive(Debug, Default, Deserialize)]
struct MeiConfigDiscover {
    #[serde(default)]
    skip_directories: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MeiConfigDisk {
    #[serde(default)]
    discover: MeiConfigDiscover,
}

fn baked_skip_dir_names() -> HashSet<String> {
    ["node_modules", ".git", "target", "dist"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn collect_skip_directory_names(source_root: &FsPath, app_root: &FsPath) -> HashSet<String> {
    let mut out = baked_skip_dir_names();
    for dir in app_root.ancestors() {
        if dir == source_root {
            break;
        }
        let path = dir.join(".mei-config.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<MeiConfigDisk>(&raw) {
                for d in cfg.discover.skip_directories {
                    let t = d.trim().trim_matches('/').replace('\\', "/");
                    if !t.is_empty() && !t.contains('/') {
                        out.insert(t);
                    }
                }
            }
        }
    }
    out
}

fn relative_path_has_dot_segment(relative: &str) -> bool {
    relative
        .split('/')
        .any(|seg| !seg.is_empty() && seg.starts_with('.'))
}

pub(super) fn collect_mei_file_entries(source_root: &FsPath, app_root: &FsPath) -> Vec<MeiFileEntry> {
    let skip_dirs = collect_skip_directory_names(source_root, app_root);
    let skip = skip_dirs.clone();
    let mut files = WalkDir::new(app_root)
        .into_iter()
        .filter_entry(move |entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') || name.starts_with('_') {
                return false;
            }
            !skip.contains(name.as_ref())
        })
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy();
            !name.starts_with('.')
        })
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei"))
        .filter_map(|entry| {
            let relative_path = entry
                .path()
                .strip_prefix(app_root)
                .ok()
                .and_then(|path| path.to_str())
                .map(|path| path.replace('\\', "/"))?;
            if relative_path_has_dot_segment(&relative_path) {
                return None;
            }
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
