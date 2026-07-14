use super::prelude::*;
use super::*;

pub(crate) fn evict_compile_cache_entries_for_write(
    cache: &mut HashMap<String, CachedCompiledApp>,
    source_root: &Path,
    app_id: &str,
) {
    let app_prefix = format!("{}#{app_id}|", normalize_path(source_root));
    let before = cache.len();
    let target_len = compile_cache_max_entries().saturating_sub(16).max(1);
    let mut overflow = cache.len().saturating_sub(target_len);
    if overflow > 0 {
        let app_keys = cache
            .keys()
            .filter(|key| key.starts_with(app_prefix.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for key in app_keys.into_iter().take(overflow) {
            cache.remove(&key);
            overflow = overflow.saturating_sub(1);
            if overflow == 0 {
                break;
            }
        }
    }
    if overflow > 0 {
        let keys = cache.keys().cloned().collect::<Vec<_>>();
        for key in keys.into_iter().take(overflow) {
            cache.remove(&key);
            overflow = overflow.saturating_sub(1);
            if overflow == 0 {
                break;
            }
        }
    }
    let removed = before.saturating_sub(cache.len());
    if removed > 0 {
        tracing::info!(
            app_id = %app_id,
            removed,
            cache_size = cache.len(),
            "compile cache reached max size; evicted oldest-ish entries"
        );
    }
}

fn remove_stale_compile_cache_entry(cache_key: &str) {
    if let Ok(mut cache) = compile_cache().write() {
        cache.remove(cache_key);
    }
}

pub(crate) fn validate_cached_entry(
    source_root: &Path,
    app_id: &str,
    entry: &CachedCompiledApp,
    components_root: &Path,
    options: &CompileOptions,
) -> Option<PeekCompileCacheHitShared> {
    if !artifact_matches_compile_scene_request(options, entry.compiled.as_ref()) {
        return None;
    }
    if watched_files_are_fresh(source_root, app_id, entry, components_root) {
        return Some(PeekCompileCacheHitShared {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "watch_set".to_string(),
            cache_validation: "watch_set".to_string(),
        });
    }
    let revision_stamp = compile_revision(source_root, app_id, options, components_root);
    (entry.compile_revision == revision_stamp.token).then(|| PeekCompileCacheHitShared {
        compiled: entry.compiled.clone(),
        compile_revision: revision_stamp.token,
        revision_scope: revision_stamp.scope.to_string(),
        cache_validation: "focused_token".to_string(),
    })
}

pub fn compile_cache_key(source_root: &Path, app_id: &str, options: &CompileOptions) -> String {
    let scene = options.scene.as_deref().unwrap_or("");
    let focus = options.preview_target.as_deref().unwrap_or("");
    let (scene_key, focus_key) = if canonical_artifact_persist_enabled() {
        let has_scene = !scene.trim().is_empty();
        let has_target = !focus.trim().is_empty();
        if has_scene && has_target {
            ("", focus)
        } else {
            (scene, focus)
        }
    } else {
        (scene, focus)
    };
    format!(
        "{}#{app_id}|v6|gen={COMPILE_SEMANTICS_GENERATION}|scene={scene_key}|focus={focus_key}",
        normalize_path(source_root),
    )
}

pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(crate) fn watched_files_are_fresh(
    source_root: &Path,
    app_id: &str,
    entry: &CachedCompiledApp,
    components_root: &Path,
) -> bool {
    if entry.watched_files.is_empty() {
        return false;
    }
    if entry.components_revision != components_revision(components_root) {
        return false;
    }
    let app_root = resolve_app_root(source_root, app_id);
    entry.watched_files.iter().all(|watched| {
        let path = if mei_lang_kernel::is_app_mei_source_rel(watched.rel_path.as_str()) {
            mei_lang_kernel::resolve_app_mei_file_path(&app_root, watched.rel_path.as_str())
        } else {
            app_root.join(&watched.rel_path)
        };
        let Ok(metadata) = std::fs::metadata(&path) else {
            return false;
        };
        if let Some(expected_signature) = watched.content_signature.as_deref() {
            let current_signature =
                mei_lang_kernel::source_file_content_signature(path.as_path(), &watched.rel_path);
            return current_signature == expected_signature;
        }
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|dur| dur.as_millis())
            .unwrap_or(0);
        metadata.len() == watched.size_bytes && modified_ms == watched.modified_ms
    })
}

pub(crate) fn default_scene_alias_keys(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
) -> Vec<String> {
    if options.preview_target.is_some() {
        return Vec::new();
    }
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let default_stage = compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .map(|route| route.scene_id.trim())
        .filter(|scene| !scene.is_empty());
    let (Some(active_scene), Some(default_scene)) = (active_scene, default_stage) else {
        return Vec::new();
    };
    if active_scene != default_scene {
        return Vec::new();
    }
    let primary_key = compile_cache_key(source_root, app_id, options);
    let default_key = compile_cache_key(
        source_root,
        app_id,
        &CompileOptions {
            scene: None,
            preview_target: None,
            ..Default::default()
        },
    );
    let explicit_default_key = compile_cache_key(
        source_root,
        app_id,
        &CompileOptions {
            scene: Some(default_scene.to_string()),
            preview_target: None,
            ..Default::default()
        },
    );
    [default_key, explicit_default_key]
        .into_iter()
        .filter(|candidate| candidate != &primary_key)
        .collect()
}

pub fn peek_compile_cache(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompiledApp> {
    peek_compile_cache_shared(source_root, app_id, options, components_root)
        .map(|compiled| (*compiled).clone())
}

pub fn peek_compile_cache_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<Arc<CompiledApp>> {
    peek_compile_cache_hit_shared(source_root, app_id, options, components_root)
        .map(|hit| hit.compiled)
}

pub fn peek_compile_cache_hit(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<PeekCompileCacheHit> {
    peek_compile_cache_hit_shared(source_root, app_id, options, components_root)
        .map(PeekCompileCacheHitShared::into_owned)
}

pub fn peek_compile_cache_hit_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<PeekCompileCacheHitShared> {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let hit = {
        let cache = compile_cache().read().ok()?;
        let entry = cache.get(&cache_key)?;
        validate_cached_entry(source_root, app_id, entry, components_root, options)
    };
    if hit.is_none() {
        remove_stale_compile_cache_entry(&cache_key);
    }
    hit
}

pub fn is_compile_inflight(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    singleflight::is_compile_inflight(source_root, app_id, options)
}

pub fn clear_compile_cache_for_app(source_root: &Path, app_id: &str) -> usize {
    let Ok(mut cache) = compile_cache().write() else {
        tracing::warn!(app_id = %app_id, "compile cache lock poisoned during clear");
        return 0;
    };
    let prefix = format!("{}#{app_id}|", normalize_path(source_root));
    let before = cache.len();
    cache.retain(|key, _| !key.starts_with(prefix.as_str()));
    before.saturating_sub(cache.len())
}

pub fn clear_compiled_app_artifacts_for_app(source_root: &Path, app_id: &str) -> usize {
    let app_root = resolve_app_root(source_root, app_id);
    let root = compiled_app_artifact_root(&app_root);
    let manifests_dir = root.join("manifests").join(COMPILED_APP_ARTIFACT_KIND);
    let artifacts_dir = root.join("artifacts").join(COMPILED_APP_ARTIFACT_KIND);
    let removed = count_files_recursively(&manifests_dir) + count_files_recursively(&artifacts_dir);
    let _ = std::fs::remove_dir_all(manifests_dir);
    let _ = std::fs::remove_dir_all(artifacts_dir);
    removed
}

pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    kernel_resolve_components_root(source_root)
}
