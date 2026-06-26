use super::prelude::*;
use super::*;

/// Restore serde-skipped runtime metric fields from persisted compiled-app sidecars.
///
/// ScenePayload artifacts serialize `CompiledApp` without `runtime_metric_defs`
/// (`#[serde(skip)]` on `DatasetView`). MCG assemble-only paths must call this
/// before serving access traffic or prebuild eval.
pub fn hydrate_compiled_app_from_disk_artifacts(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &mut CompiledApp,
) -> bool {
    if !compiled_app_artifact_enabled() {
        return false;
    }
    let app_root = resolve_app_root(source_root, app_id);
    for scope in compiled_app_artifact_lookup_scopes(&app_root, options) {
        let Ok(Some((_, artifact))) = read_json_artifact::<CompiledAppDiskArtifact>(
            &app_root,
            COMPILED_APP_ARTIFACT_KIND,
            COMPILED_APP_ARTIFACT_NAME,
            &scope,
        ) else {
            continue;
        };
        if artifact.dataset_runtime_payloads.is_empty() {
            continue;
        }
        hydrate_compiled_app_runtime_payloads(compiled, &artifact.dataset_runtime_payloads);
        return true;
    }
    false
}
pub fn probe_compiled_app_manifest_identity(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
) -> Option<String> {
    if !compiled_app_artifact_enabled() {
        return None;
    }
    let app_root = resolve_app_root(source_root, app_id);
    let manifest = read_artifact_manifest(
        app_root.as_path(),
        COMPILED_APP_ARTIFACT_KIND,
        COMPILED_APP_ARTIFACT_NAME,
        scope,
    )
    .ok()??;
    Some(compiled_app_manifest_identity(&manifest))
}

pub(crate) fn load_compiled_app_artifact_at_scope(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
    app_root: &Path,
    scope: &WorldScope,
    artifact_started: Instant,
) -> Option<(PeekCompileCacheHitShared, u64)> {
    let Ok(Some((manifest, mut artifact))) = read_json_artifact::<CompiledAppDiskArtifact>(
        app_root,
        COMPILED_APP_ARTIFACT_KIND,
        COMPILED_APP_ARTIFACT_NAME,
        scope,
    ) else {
        return None;
    };
    if artifact.schema_version != COMPILED_APP_ARTIFACT_SCHEMA_VERSION
        && artifact.schema_version != COMPILED_APP_ARTIFACT_SLIM_SCHEMA_VERSION
    {
        return None;
    }
    hydrate_compiled_app_runtime_payloads(
        &mut artifact.compiled,
        &artifact.dataset_runtime_payloads,
    );
    if artifact.schema_version == COMPILED_APP_ARTIFACT_SCHEMA_VERSION
        && access_slim_artifacts_enabled()
    {
        strip_loaded_compiled_app_for_access(&mut artifact.compiled);
    }
    artifact.compiled.app_root = app_root.display().to_string();
    let cached = CachedCompiledApp {
        compile_revision: artifact.compile_revision.clone(),
        watched_files: manifest
            .watched_files
            .iter()
            .map(|watched| CompileWatchedFile {
                rel_path: watched.rel_path.clone(),
                modified_ms: watched.modified_ms,
                size_bytes: watched.size_bytes,
                content_signature: watched.content_signature.clone(),
            })
            .collect(),
        components_revision: manifest.components_revision,
        compiled: Arc::new(artifact.compiled),
    };
    if !artifact_matches_compile_scene_request(options, &cached.compiled) {
        return None;
    }
    let hit = validate_cached_entry(source_root, app_id, &cached, components_root, options)?;
    let artifact_load_ms = elapsed_ms(artifact_started);
    store_compile_cache_entry(
        &compile_cache_key(source_root, app_id, options),
        source_root,
        app_id,
        options,
        &cached.compile_revision,
        &cached.watched_files,
        cached.components_revision,
        cached.compiled.clone(),
    );
    Some((hit, artifact_load_ms))
}

pub(crate) fn maybe_load_compiled_app_artifact(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<(PeekCompileCacheHitShared, u64)> {
    if !compiled_app_artifact_enabled() {
        return None;
    }
    let artifact_started = Instant::now();
    let app_root = resolve_app_root(source_root, app_id);
    let mut seen = BTreeMap::new();
    for scope in compiled_app_artifact_lookup_scopes(&app_root, options) {
        let key = format!(
            "{}|{}",
            scope.scene_id.as_deref().unwrap_or(""),
            scope.target_file.as_deref().unwrap_or("")
        );
        if !seen.insert(key, ()).is_none() {
            continue;
        }
        if let Some(hit) = load_compiled_app_artifact_at_scope(
            source_root,
            app_id,
            options,
            components_root,
            &app_root,
            &scope,
            artifact_started,
        ) {
            return Some(hit);
        }
    }
    None

}
