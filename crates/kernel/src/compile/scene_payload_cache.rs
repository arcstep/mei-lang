//! 统一 L2：`compile_scene_payload_for_target` 进程内缓存。
//!
//! 覆盖 route 发现、official 编译、catalog、embed 预览等全部调用方，避免同一 `.mei` 在单次 compile 内被重复编译。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

const MAX_SCENE_PAYLOAD_CACHE_ENTRIES: usize = 128;

pub(crate) fn file_mtime_ms(path: &Path) -> u128 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn directory_latest_mtime_ms(path: &Path) -> u128 {
    if !path.exists() {
        return 0;
    }
    let mut latest = file_mtime_ms(path);
    for entry in WalkDir::new(path).into_iter().flatten() {
        let name = entry.file_name().to_string_lossy();
        if entry.depth() > 0
            && matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | ".mei" | "__pycache__"
            )
        {
            continue;
        }
        latest = latest.max(file_mtime_ms(entry.path()));
    }
    latest
}

fn components_revision(source_root: &Path) -> u128 {
    directory_latest_mtime_ms(source_root)
}

fn normalize_target_file(target_file: &str) -> String {
    target_file.trim().trim_start_matches("./").replace('\\', "/")
}

pub(crate) fn scene_payload_cache_key(
    app_root: &Path,
    source_root: &Path,
    target_file: &str,
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
        "v{SCENE_PAYLOAD_CACHE_VERSION}|{}|{target_file}|{}|{}|{}",
        app_root.display(),
        file_mtime_ms(&target_path),
        file_mtime_ms(&main_path),
        components_revision(source_root),
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
) -> CompiledScenePayload {
    if let Some(key) = scene_payload_cache_key(app_root, source_root, target_file) {
        if let Some(payload) = take_scene_payload_cache(&key) {
            return payload;
        }
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
    super::entry_payload::compile_scene_payload_for_target_uncached(
        app_root,
        app_decls,
        asset_map,
        target_file,
        route_meta,
        scene_registry,
    )
}

/// 按 target 文件索引 official_results，避免 route 发现阶段已编译的文件在 official 循环中再编一次。
pub(super) fn index_official_payloads_by_target(
    payloads: &[(String, CompiledScenePayload)],
) -> BTreeMap<String, CompiledScenePayload> {
    payloads
        .iter()
        .map(|(target, payload)| (target.clone(), payload.clone()))
        .collect()
}

pub(crate) fn clear_scene_payload_cache_for_tests() {
    if let Ok(mut cache) = SCENE_PAYLOAD_CACHE.lock() {
        cache.clear();
    }
}

pub(crate) fn scene_payload_cache_len_for_tests() -> usize {
    SCENE_PAYLOAD_CACHE.lock().map(|c| c.len()).unwrap_or(0)
}

pub(super) fn resolve_source_root(app_root: &Path) -> PathBuf {
    app_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| app_root.to_path_buf())
}
