use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mei_lang_kernel::{
    compile_revision_plan_from_root_with_options, resolve_app_root, CompileOptions,
    CompileWatchedFile,
};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub(crate) struct CompileRevisionStamp {
    pub(crate) token: String,
    pub(crate) scope: &'static str,
    pub(crate) watched_files: Vec<CompileWatchedFile>,
    pub(crate) components_revision: u128,
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

pub(crate) fn compile_revision(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> CompileRevisionStamp {
    let app_root = resolve_app_root(source_root, app_id);
    if let Ok(plan) = compile_revision_plan_from_root_with_options(source_root, &app_root, options)
    {
        return CompileRevisionStamp {
            token: plan.token,
            scope: "focused_graph",
            watched_files: plan.watched_files,
            components_revision: plan.components_revision,
        };
    }
    compile_revision_fallback(&app_root, components_root)
}

fn compile_revision_fallback(app_root: &Path, components_root: &Path) -> CompileRevisionStamp {
    if compile_revision_mode() == RevisionMode::Full {
        let app_mtime = directory_latest_full_modified_ms(app_root).unwrap_or(0);
        let components_mtime = directory_latest_full_modified_ms(components_root).unwrap_or(0);
        return CompileRevisionStamp {
            token: app_mtime.max(components_mtime).to_string(),
            scope: "full_mtime",
            watched_files: Vec::new(),
            components_revision: components_mtime,
        };
    }
    let app_mtime = directory_latest_modified_ms(app_root, RevisionScope::App).unwrap_or(0);
    let components_mtime =
        directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0);
    CompileRevisionStamp {
        token: app_mtime.max(components_mtime).to_string(),
        scope: "relevant_mtime",
        watched_files: Vec::new(),
        components_revision: components_mtime,
    }
}

pub(crate) fn components_revision(components_root: &Path) -> u128 {
    if compile_revision_mode() == RevisionMode::Full {
        return directory_latest_full_modified_ms(components_root).unwrap_or(0);
    }
    directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0)
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
    let mut latest = std::fs::metadata(path)
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
    let mut latest = std::fs::metadata(path)
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
        RevisionScope::Components => is_component_manifest(path) || is_component_js(path),
    }
}

fn is_component_manifest(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("manifest.json")
        && !name.eq_ignore_ascii_case("component.manifest.json")
    {
        return false;
    }
    is_components_tree_path(&normalize_path(path))
}

fn is_component_js(path: &Path) -> bool {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
    {
        return false;
    }
    is_components_tree_path(&normalize_path(path))
}

fn is_components_tree_path(normalized: &str) -> bool {
    normalized.contains("/.stock/components/") || normalized.contains("/_components/")
}

pub(crate) fn normalize_path(path: &Path) -> String {
    PathBuf::from(path).to_string_lossy().replace('\\', "/")
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}

#[cfg(test)]
mod tests {
    use super::{is_component_js, is_component_manifest};
    use std::path::Path;

    #[test]
    fn stock_components_manifest_and_js_are_revision_relevant() {
        let manifest = Path::new("/ws/zhifa/.stock/components/mei/manifest.json");
        let script = Path::new("/ws/zhifa/.stock/components/mei/text.js");
        assert!(is_component_manifest(manifest));
        assert!(is_component_js(script));
    }

    #[test]
    fn legacy_components_manifest_still_counts() {
        let manifest = Path::new("/ws/zhifa/_components/mei/component.manifest.json");
        assert!(is_component_manifest(manifest));
    }
}
