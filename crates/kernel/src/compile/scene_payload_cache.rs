//! 统一 L2：`compile_scene_payload_for_target` 进程内缓存。
//!
//! 覆盖 route 发现、official 编译、catalog、embed 预览等全部调用方，避免同一 `.mei` 在单次 compile 内被重复编译。

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde_json::Value;
use walkdir::WalkDir;

use crate::model::{CompiledSceneRoute, ComponentAsset};
use crate::typed_refs::SceneRegistry;

use super::entry_payload::CompiledScenePayload;

/// 物化/编译语义变更时递增，使旧缓存条目失效。
pub const SCENE_PAYLOAD_CACHE_VERSION: u32 = 3;

pub fn scene_payload_cache_epoch() -> String {
    format!("l2v{SCENE_PAYLOAD_CACHE_VERSION}")
}

static SCENE_PAYLOAD_CACHE: Mutex<BTreeMap<String, CompiledScenePayload>> =
    Mutex::new(BTreeMap::new());
static SCENE_PAYLOAD_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static SCENE_PAYLOAD_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

const MAX_SCENE_PAYLOAD_CACHE_ENTRIES: usize = 128;

pub(crate) fn file_mtime_ms(path: &Path) -> u128 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[derive(Clone, Copy)]
enum RevisionScope {
    App,
    Components,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RevisionMode {
    Relevant,
    Full,
}

fn compile_revision_mode() -> RevisionMode {
    let raw = env::var("MEI_COMPILE_REVISION_MODE").unwrap_or_default();
    if raw.trim().eq_ignore_ascii_case("full") {
        RevisionMode::Full
    } else {
        RevisionMode::Relevant
    }
}

fn directory_latest_full_mtime_ms(path: &Path) -> u128 {
    if !path.exists() {
        return 0;
    }
    let mut latest = file_mtime_ms(path);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        latest = latest.max(file_mtime_ms(entry.path()));
    }
    latest
}

fn directory_latest_relevant_mtime_ms(path: &Path, scope: RevisionScope) -> u128 {
    if !path.exists() {
        return 0;
    }
    let mut latest = file_mtime_ms(path);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        if entry.file_type().is_file() && is_revision_relevant(entry.path(), scope) {
            latest = latest.max(file_mtime_ms(entry.path()));
        }
    }
    latest
}

fn resolve_components_root(source_root: &Path) -> PathBuf {
    let local = source_root.join("_components");
    if local.exists() {
        return local;
    }
    if let Some(parent) = source_root.parent() {
        let shared = parent.join("_components");
        if shared.exists() {
            return shared;
        }
    }
    local
}

fn components_revision(source_root: &Path) -> u128 {
    if compile_revision_mode() == RevisionMode::Full {
        return directory_latest_full_mtime_ms(&resolve_components_root(source_root));
    }
    directory_latest_relevant_mtime_ms(
        &resolve_components_root(source_root),
        RevisionScope::Components,
    )
}

fn app_revision(app_root: &Path) -> u128 {
    if compile_revision_mode() == RevisionMode::Full {
        return directory_latest_full_mtime_ms(app_root);
    }
    directory_latest_relevant_mtime_ms(app_root, RevisionScope::App)
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
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.contains("/_components/")
}

fn normalize_target_file(target_file: &str) -> String {
    target_file
        .trim()
        .trim_start_matches("./")
        .replace('\\', "/")
}

pub(crate) fn scene_payload_cache_key(
    app_root: &Path,
    source_root: &Path,
    target_file: &str,
    dependency_fingerprint: Option<&str>,
) -> Option<String> {
    let target_file = normalize_target_file(target_file);
    if target_file.is_empty() {
        return None;
    }
    let target_path = app_root.join(&target_file);
    if !target_path.is_file() {
        return None;
    }
    let main_path = app_root.join("main.mei");
    Some(format!(
        "v{SCENE_PAYLOAD_CACHE_VERSION}|{}|{target_file}|{}|{}|{}|{}",
        app_root.display(),
        file_mtime_ms(&target_path),
        file_mtime_ms(&main_path),
        app_revision(app_root).max(components_revision(source_root)),
        dependency_fingerprint.unwrap_or("-"),
    ))
}

fn store_scene_payload_cache(key: String, payload: CompiledScenePayload) {
    if let Ok(mut cache) = SCENE_PAYLOAD_CACHE.lock() {
        if cache.len() >= MAX_SCENE_PAYLOAD_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, payload);
    }
}

fn take_scene_payload_cache(key: &str) -> Option<CompiledScenePayload> {
    SCENE_PAYLOAD_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())
}

/// 带 L2 缓存的 scene payload 编译入口；所有内核路径应经此函数调用。
pub(super) fn compile_scene_payload_for_target(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
    dependency_fingerprint: Option<&str>,
) -> CompiledScenePayload {
    if let Some(key) =
        scene_payload_cache_key(app_root, source_root, target_file, dependency_fingerprint)
    {
        if let Some(payload) = take_scene_payload_cache(&key) {
            SCENE_PAYLOAD_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return payload;
        }
        SCENE_PAYLOAD_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        let payload = super::entry_payload::compile_scene_payload_for_target_uncached(
            app_root,
            app_decls,
            asset_map,
            target_file,
            route_meta,
            scene_registry,
        );
        store_scene_payload_cache(key, payload.clone());
        return payload;
    }
    SCENE_PAYLOAD_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    super::entry_payload::compile_scene_payload_for_target_uncached(
        app_root,
        app_decls,
        asset_map,
        target_file,
        route_meta,
        scene_registry,
    )
}

pub(crate) fn scene_payload_cache_metrics_snapshot() -> (u64, u64) {
    (
        SCENE_PAYLOAD_CACHE_HITS.load(Ordering::Relaxed),
        SCENE_PAYLOAD_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn clear_scene_payload_cache_for_tests() {
    if let Ok(mut cache) = SCENE_PAYLOAD_CACHE.lock() {
        cache.clear();
    }
}

#[cfg(test)]
pub(crate) fn scene_payload_cache_len_for_tests() -> usize {
    SCENE_PAYLOAD_CACHE.lock().map(|c| c.len()).unwrap_or(0)
}
