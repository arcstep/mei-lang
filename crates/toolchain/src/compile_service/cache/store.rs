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

pub(crate) fn artifact_matches_compile_scene_request(options: &CompileOptions, compiled: &CompiledApp) -> bool {
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

pub(crate) fn list_compiled_app_scopes_for_target(app_root: &Path, target_file: &str) -> Vec<WorldScope> {
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

pub(crate) fn compiled_app_artifact_context(
    app_id: &str,
    options: &CompileOptions,
    active_scene_id: Option<String>,
    active_target_file: String,
    revision_stamp: &revision::CompileRevisionStamp,
) -> ArtifactWriteContext {
    ArtifactWriteContext {
        app_id: app_id.to_string(),
        artifact_kind: COMPILED_APP_ARTIFACT_KIND.to_string(),
        artifact_name: COMPILED_APP_ARTIFACT_NAME.to_string(),
        scope: compiled_app_artifact_scope(options),
        active_scene_id,
        active_target_file,
        revision_token: revision_stamp.token.clone(),
        components_revision: revision_stamp.components_revision,
        watched_files: revision_stamp
            .watched_files
            .iter()
            .map(ArtifactWatchedFile::from)
            .collect(),
    }
}

pub(crate) fn compiled_app_artifact_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_build_root(app_root)
}

pub(crate) fn extract_dataset_runtime_payloads(
    compiled: &CompiledApp,
) -> BTreeMap<String, DatasetRuntimePayload> {
    let mut payloads = BTreeMap::new();
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.runtime_metric_defs.is_empty()
            && dataset.runtime_analysis_graph.nodes.is_empty()
            && dataset.runtime_analysis_contracts.is_empty()
        {
            continue;
        }
        payloads.insert(
            resource.id.clone(),
            DatasetRuntimePayload {
                runtime_metric_defs: dataset.runtime_metric_defs.clone(),
                runtime_analysis_graph: dataset.runtime_analysis_graph.clone(),
                runtime_analysis_contracts: dataset.runtime_analysis_contracts.clone(),
            },
        );
    }
    payloads
}

pub(crate) fn build_assembly_inputs(
    compiled: &CompiledApp,
    payloads: &BTreeMap<String, DatasetRuntimePayload>,
    revision_stamp: &revision::CompileRevisionStamp,
) -> Vec<AssemblyInputDiskRecord> {
    let mut inputs = Vec::new();
    inputs.push(AssemblyInputDiskRecord {
        kind: "scene_payload".to_string(),
        key: compiled.active_target_file.clone(),
        revision: format!(
            "nr:{}:{}",
            mei_lang_kernel::scene_payload_cache_epoch(),
            revision_stamp.token
        ),
    });
    for (owner_id, payload) in payloads {
        if payload.runtime_metric_defs.is_empty() {
            continue;
        }
        let serialized = serde_json::to_string(&payload.runtime_metric_defs).unwrap_or_default();
        let fingerprint = stable_assembly_hash(&serialized);
        inputs.push(AssemblyInputDiskRecord {
            kind: "metric_def_bundle".to_string(),
            key: owner_id.clone(),
            revision: format!("mdb:{fingerprint}"),
        });
    }
    inputs
}

pub(crate) fn stable_assembly_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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

pub(crate) fn write_compiled_app_artifact_value(
    app_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
    revision_stamp: &revision::CompileRevisionStamp,
    value: &Value,
) {
    let context = compiled_app_artifact_context(
        app_id,
        options,
        compiled.active_scene.clone(),
        compiled.active_target_file.clone(),
        revision_stamp,
    );
    if let Err(error) = write_json_artifact(app_root, &context, value) {
        tracing::warn!(
            app_id = %app_id,
            scene = %options.scene.as_deref().unwrap_or(""),
            focus = %options.preview_target.as_deref().unwrap_or(""),
            error = %error,
            "failed to write compiled app artifact"
        );
    }
}

pub(crate) fn maybe_write_compiled_app_artifact(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    revision_stamp: &revision::CompileRevisionStamp,
    compiled: &CompiledApp,
) {
    if !compiled_app_artifact_enabled() {
        return;
    }
    if content_store_preferred() {
        return;
    }
    if !should_persist_compiled_app_artifact(
        options.scene.as_deref(),
        options.preview_target.as_deref(),
    ) {
        return;
    }
    let app_root = resolve_app_root(source_root, app_id);
    let payloads = extract_dataset_runtime_payloads(compiled);
    let use_slim = access_slim_artifacts_enabled();
    let compiled_body = if use_slim {
        slim_compiled_app_for_access(compiled)
    } else {
        compiled.clone()
    };
    let artifact = CompiledAppDiskArtifact {
        schema_version: if use_slim {
            COMPILED_APP_ARTIFACT_SLIM_SCHEMA_VERSION.to_string()
        } else {
            COMPILED_APP_ARTIFACT_SCHEMA_VERSION.to_string()
        },
        compile_revision: revision_stamp.token.clone(),
        revision_scope: revision_stamp.scope.to_string(),
        compiled: compiled_body,
        dataset_runtime_payloads: payloads.clone(),
        assembly_inputs: build_assembly_inputs(compiled, &payloads, revision_stamp),
        access_slim: use_slim,
    };
    if let Ok(value) = serde_json::to_value(&artifact) {
        write_compiled_app_artifact_value(
            &app_root,
            app_id,
            options,
            compiled,
            revision_stamp,
            &value,
        );
        if !canonical_artifact_persist_enabled() {
            let scene_only_requested = options
                .scene
                .as_deref()
                .map(str::trim)
                .is_some_and(|scene| !scene.is_empty())
                && options
                    .preview_target
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(str::is_empty);
            if !scene_only_requested {
                let scene_only = CompileOptions {
                    scene: options.scene.clone(),
                    preview_target: None,
                };
                write_compiled_app_artifact_value(
                    &app_root,
                    app_id,
                    &scene_only,
                    compiled,
                    revision_stamp,
                    &value,
                );
            }
        }
    }
}
