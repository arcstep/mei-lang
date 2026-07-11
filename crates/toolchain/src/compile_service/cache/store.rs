use super::prelude::*;
use super::*;

pub(crate) fn compiled_app_artifact_enabled() -> bool {
    !env_flag_enabled("MEI_DISABLE_COMPILED_APP_ARTIFACTS")
}

pub(crate) fn compiled_app_artifact_scope(options: &CompileOptions) -> WorldScope {
    WorldScope {
        scene_id: options.scene.clone(),
        target_file: options.preview_target.clone(),
    }
}

pub(crate) fn artifact_matches_compile_scene_request(
    options: &CompileOptions,
    compiled: &CompiledApp,
) -> bool {
    let requested_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty());
    let requested_scene = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    if let Some(requested_target) = requested_target {
        let active_target = compiled.active_target_file.trim();
        if active_target != requested_target {
            return false;
        }
        // Board overlay requests carry both export scene id and board target file.
        // Parent-scene artifacts (e.g. home + board.mei warmup scope) may embed board
        // assembly metadata but still expose the parent resource table; do not reuse them.
        if let Some(_requested_scene) = requested_scene {
            if canonical_artifact_persist_enabled() {
                return true;
            }
            return compiled
                .active_scene
                .as_deref()
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                == Some(_requested_scene);
        }
        return true;
    }
    let Some(requested_scene) = requested_scene else {
        return true;
    };
    compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty())
        == Some(requested_scene)
}

pub(crate) fn normalized_scope_target(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(str::to_string)
}

pub(crate) fn list_compiled_app_scopes_for_target(
    app_root: &Path,
    target_file: &str,
) -> Vec<WorldScope> {
    let target_file = target_file.trim();
    if target_file.is_empty() {
        return Vec::new();
    }
    let manifests_dir = compiled_app_artifact_root(app_root)
        .join("manifests")
        .join(COMPILED_APP_ARTIFACT_KIND);
    let Ok(entries) = fs::read_dir(&manifests_dir) else {
        return Vec::new();
    };
    let mut scopes = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<ArtifactStoreManifest>(&text) else {
            continue;
        };
        if manifest.artifact_kind != COMPILED_APP_ARTIFACT_KIND
            || manifest.artifact_name != COMPILED_APP_ARTIFACT_NAME
        {
            continue;
        }
        let Some(scope_target) = normalized_scope_target(manifest.scope.target_file.as_deref())
        else {
            continue;
        };
        if scope_target != target_file {
            continue;
        }
        let key = format!(
            "{}|{}",
            manifest.scope.scene_id.as_deref().unwrap_or(""),
            scope_target
        );
        if seen.insert(key, ()).is_some() {
            continue;
        }
        scopes.push(manifest.scope);
    }
    scopes
}

pub(crate) fn compiled_app_artifact_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_build_root(app_root)
}

pub(crate) fn hydrate_compiled_app_runtime_payloads(
    compiled: &mut CompiledApp,
    payloads: &BTreeMap<String, DatasetRuntimePayload>,
) {
    for resource in &mut compiled.resources {
        let Some(payload) = payloads.get(&resource.id) else {
            continue;
        };
        let Some(dataset) = resource.dataset.as_mut() else {
            continue;
        };
        dataset.runtime_metric_defs = payload.runtime_metric_defs.clone();
        dataset.runtime_analysis_graph = payload.runtime_analysis_graph.clone();
        dataset.runtime_analysis_contracts = payload.runtime_analysis_contracts.clone();
    }
}

pub(crate) fn count_files_recursively(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|child| {
            if child.is_file() {
                1
            } else if child.is_dir() {
                count_files_recursively(&child)
            } else {
                0
            }
        })
        .sum()
}

pub(crate) fn store_compile_cache_entry(
    cache_key: &str,
    source_root: &Path,
    app_id: &str,
    alias_options: &CompileOptions,
    compile_revision: &str,
    watched_files: &[CompileWatchedFile],
    components_revision: u128,
    compiled: Arc<CompiledApp>,
) {
    let write_lock_started = Instant::now();
    if let Ok(mut cache) = compile_cache().write() {
        let _ = elapsed_ms(write_lock_started);
        if cache.len() >= compile_cache_max_entries() {
            evict_compile_cache_entries_for_write(&mut cache, source_root, app_id);
        }
        let cache_entry = CachedCompiledApp {
            compile_revision: compile_revision.to_string(),
            watched_files: watched_files.to_vec(),
            components_revision,
            compiled: compiled.clone(),
        };
        cache.insert(cache_key.to_string(), cache_entry.clone());
        for alias_key in default_scene_alias_keys(source_root, app_id, alias_options, &compiled) {
            cache.insert(alias_key, cache_entry.clone());
        }
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during write; skip cache store"
        );
    }
}

pub(crate) fn maybe_write_compiled_app_artifact(
    _source_root: &Path,
    _app_id: &str,
    _options: &CompileOptions,
    _revision_stamp: &revision::CompileRevisionStamp,
    _compiled: &CompiledApp,
) {
    // 1.3.0: per-scope compiled_app artifacts are retired; L3 truth is MCG + Content Store.
}
