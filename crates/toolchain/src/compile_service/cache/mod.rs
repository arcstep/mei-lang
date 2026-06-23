mod revision;
mod singleflight;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use mei_lang_kernel::{
    compile_app_with_options, compile_app_with_options_and_revision, resolve_app_root,
    AnalysisGraph, CompileOptions, CompileWatchedFile, CompiledApp, COMPILE_SEMANTICS_GENERATION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mei_lang_kernel::resolve_components_root as kernel_resolve_components_root;

use crate::artifact_store::{
    read_json_artifact, write_json_artifact, ArtifactWatchedFile, ArtifactWriteContext,
};
use crate::types::WorldScope;
pub use singleflight::env_flag_enabled;

use revision::{compile_revision, components_revision, normalize_path};
use singleflight::{
    compile_singleflight_enabled, finish_compile_inflight, register_compile_inflight,
    wait_for_compile_inflight,
};

#[derive(Clone)]
pub(super) struct CachedCompiledApp {
    compile_revision: String,
    watched_files: Vec<CompileWatchedFile>,
    components_revision: u128,
    compiled: Arc<CompiledApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DatasetRuntimePayload {
    #[serde(default)]
    runtime_metric_defs: BTreeMap<String, Value>,
    #[serde(default)]
    runtime_analysis_graph: AnalysisGraph,
    #[serde(default)]
    runtime_analysis_contracts: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompiledAppDiskArtifact {
    schema_version: String,
    compile_revision: String,
    revision_scope: String,
    compiled: CompiledApp,
    /// `DatasetView::runtime_*` fields are `serde(skip)` on the public model, so they must be
    /// stored alongside the compiled app for artifact reload to support runtime metric eval.
    #[serde(default)]
    dataset_runtime_payloads: BTreeMap<String, DatasetRuntimePayload>,
}

const COMPILED_APP_ARTIFACT_SCHEMA_VERSION: &str = "mei-compiled-app-artifact-v3";
const COMPILED_APP_ARTIFACT_KIND: &str = "compiled_app";
const COMPILED_APP_ARTIFACT_NAME: &str = "compiled_app";

#[derive(Clone)]
pub struct CompileWithCacheOutcome {
    pub compiled: CompiledApp,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct CompileWithCacheOutcomeShared {
    pub compiled: Arc<CompiledApp>,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct CompileWithCacheFailure {
    pub error: anyhow::Error,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct PeekCompileCacheHit {
    pub compiled: CompiledApp,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
}

pub struct PeekCompileCacheHitShared {
    pub compiled: Arc<CompiledApp>,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
}

impl CompileWithCacheOutcomeShared {
    fn into_owned(self) -> CompileWithCacheOutcome {
        CompileWithCacheOutcome {
            compiled: (*self.compiled).clone(),
            cache_hit: self.cache_hit,
            artifact_cache_hit: self.artifact_cache_hit,
            compile_revision: self.compile_revision,
            revision_scope: self.revision_scope,
            cache_validation: self.cache_validation,
            cache_lookup_ms: self.cache_lookup_ms,
            artifact_load_ms: self.artifact_load_ms,
            compile_cache_lock_wait_ms: self.compile_cache_lock_wait_ms,
            compile_ms: self.compile_ms,
        }
    }
}

impl PeekCompileCacheHitShared {
    fn into_owned(self) -> PeekCompileCacheHit {
        PeekCompileCacheHit {
            compiled: (*self.compiled).clone(),
            compile_revision: self.compile_revision,
            revision_scope: self.revision_scope,
            cache_validation: self.cache_validation,
        }
    }
}

pub(super) fn compile_cache() -> &'static RwLock<HashMap<String, CachedCompiledApp>> {
    static COMPILE_CACHE: OnceLock<RwLock<HashMap<String, CachedCompiledApp>>> = OnceLock::new();
    COMPILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn compile_failure_latch() -> &'static StdMutex<HashMap<String, Instant>> {
    static COMPILE_FAILURE_LATCH: OnceLock<StdMutex<HashMap<String, Instant>>> = OnceLock::new();
    COMPILE_FAILURE_LATCH.get_or_init(|| StdMutex::new(HashMap::new()))
}

const COMPILE_FAILURE_LATCH_TTL: Duration = Duration::from_secs(45);

fn compile_cache_max_entries() -> usize {
    static MAX_ENTRIES: OnceLock<usize> = OnceLock::new();
    *MAX_ENTRIES.get_or_init(|| {
        std::env::var("MEI_COMPILE_CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10240)
    })
}

fn compiled_app_artifact_enabled() -> bool {
    !env_flag_enabled("MEI_DISABLE_COMPILED_APP_ARTIFACTS")
}

fn compiled_app_artifact_scope(options: &CompileOptions) -> WorldScope {
    WorldScope {
        scene_id: options.scene.clone(),
        target_file: options.preview_target.clone(),
    }
}

fn artifact_matches_compile_scene_request(options: &CompileOptions, compiled: &CompiledApp) -> bool {
    let Some(requested_scene) = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty())
    else {
        return true;
    };
    compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty())
        == Some(requested_scene)
}

fn compiled_app_artifact_context(
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

fn compiled_app_artifact_root(app_root: &Path) -> PathBuf {
    app_root.join(".mei")
}

fn extract_dataset_runtime_payloads(
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

fn hydrate_compiled_app_runtime_payloads(
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

fn count_files_recursively(path: &Path) -> usize {
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

fn store_compile_cache_entry(
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

fn write_compiled_app_artifact_value(
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

fn maybe_write_compiled_app_artifact(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    revision_stamp: &revision::CompileRevisionStamp,
    compiled: &CompiledApp,
) {
    if !compiled_app_artifact_enabled() {
        return;
    }
    let app_root = resolve_app_root(source_root, app_id);
    let artifact = CompiledAppDiskArtifact {
        schema_version: COMPILED_APP_ARTIFACT_SCHEMA_VERSION.to_string(),
        compile_revision: revision_stamp.token.clone(),
        revision_scope: revision_stamp.scope.to_string(),
        compiled: compiled.clone(),
        dataset_runtime_payloads: extract_dataset_runtime_payloads(compiled),
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

fn ensure_compiled_app_artifact_alias(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
    compiled: &CompiledApp,
) {
    if !compiled_app_artifact_enabled() {
        return;
    }
    let revision_stamp = compile_revision(source_root, app_id, options, components_root);
    maybe_write_compiled_app_artifact(source_root, app_id, options, &revision_stamp, compiled);
}

fn compiled_app_artifact_lookup_scopes(
    app_root: &Path,
    options: &CompileOptions,
) -> Vec<WorldScope> {
    let scene_id = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let has_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .is_some_and(|target| !target.is_empty());
    if scene_id.is_none() || has_target {
        return vec![compiled_app_artifact_scope(options)];
    }
    let scene_id = scene_id.expect("scene-only lookup requires scene id");
    let mut scopes = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let mut push_scope = |scope: WorldScope| {
        let key = format!(
            "{}|{}",
            scope.scene_id.as_deref().unwrap_or(""),
            scope.target_file.as_deref().unwrap_or("")
        );
        if seen.insert(key, ()).is_some() {
            return;
        }
        scopes.push(scope);
    };
    if let Ok(Some((_, default_artifact))) = read_json_artifact::<CompiledAppDiskArtifact>(
        app_root,
        COMPILED_APP_ARTIFACT_KIND,
        COMPILED_APP_ARTIFACT_NAME,
        &WorldScope {
            scene_id: None,
            target_file: None,
        },
    ) {
        for route in &default_artifact.compiled.scene_routes {
            if route.scene_id.trim() != scene_id {
                continue;
            }
            let target = route.target_file.trim();
            if target.is_empty() {
                continue;
            }
            push_scope(WorldScope {
                scene_id: Some(scene_id.to_string()),
                target_file: Some(target.to_string()),
            });
        }
        if default_artifact
            .compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            == Some(scene_id)
        {
            let target = default_artifact.compiled.active_target_file.trim();
            if !target.is_empty() {
                push_scope(WorldScope {
                    scene_id: Some(scene_id.to_string()),
                    target_file: Some(target.to_string()),
                });
            }
        }
    }
    push_scope(compiled_app_artifact_scope(options));
    push_scope(WorldScope {
        scene_id: None,
        target_file: None,
    });
    scopes
}

fn load_compiled_app_artifact_at_scope(
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
    if artifact.schema_version != COMPILED_APP_ARTIFACT_SCHEMA_VERSION {
        return None;
    }
    hydrate_compiled_app_runtime_payloads(
        &mut artifact.compiled,
        &artifact.dataset_runtime_payloads,
    );
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

fn maybe_load_compiled_app_artifact(
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

pub(super) fn record_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.insert(cache_key.to_string(), Instant::now());
    }
}

pub(super) fn clear_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.remove(cache_key);
    }
}

pub fn recent_compile_failure(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let Ok(guard) = compile_failure_latch().lock() else {
        return false;
    };
    guard
        .get(&cache_key)
        .is_some_and(|at| at.elapsed() < COMPILE_FAILURE_LATCH_TTL)
}

pub fn compile_app_with_cache(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    compile_app_with_cache_shared(source_root, app_id, options, components_root)
        .map(CompileWithCacheOutcomeShared::into_owned)
}

pub fn load_compile_artifact_only(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompileWithCacheOutcome> {
    load_compile_artifact_only_shared(source_root, app_id, options, components_root)
        .map(CompileWithCacheOutcomeShared::into_owned)
}

pub fn load_compile_artifact_only_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompileWithCacheOutcomeShared> {
    let cache_lookup_started = Instant::now();
    if let Some(hit) = peek_compile_cache_hit_shared(source_root, app_id, options, components_root)
    {
        return Some(CompileWithCacheOutcomeShared {
            compiled: hit.compiled,
            cache_hit: true,
            artifact_cache_hit: false,
            compile_revision: hit.compile_revision,
            revision_scope: hit.revision_scope,
            cache_validation: hit.cache_validation,
            cache_lookup_ms: elapsed_ms(cache_lookup_started),
            artifact_load_ms: 0,
            compile_cache_lock_wait_ms: 0,
            compile_ms: 0,
        });
    }
    let cache_lookup_ms = elapsed_ms(cache_lookup_started);
    let (artifact_hit, artifact_load_ms) =
        maybe_load_compiled_app_artifact(source_root, app_id, options, components_root)?;
    Some(CompileWithCacheOutcomeShared {
        compiled: artifact_hit.compiled,
        cache_hit: true,
        artifact_cache_hit: true,
        compile_revision: artifact_hit.compile_revision,
        revision_scope: artifact_hit.revision_scope,
        cache_validation: artifact_hit.cache_validation,
        cache_lookup_ms,
        artifact_load_ms,
        compile_cache_lock_wait_ms: 0,
        compile_ms: 0,
    })
}

pub fn compile_app_with_cache_shared(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcomeShared, CompileWithCacheFailure> {
    let cache_key = compile_cache_key(source_root, app_id, &options);
    if !compile_singleflight_enabled() {
        let outcome = compile_app_with_cache_uncached_path_shared(
            source_root,
            app_id,
            &cache_key,
            options,
            components_root,
        );
        match &outcome {
            Ok(_) => clear_compile_failure(&cache_key),
            Err(_) => record_compile_failure(&cache_key),
        }
        return outcome;
    }
    let singleflight_started = Instant::now();
    let Some((inflight, is_leader)) = register_compile_inflight(&cache_key) else {
        tracing::warn!(
            app_id = %app_id,
            "compile inflight map lock poisoned; fallback to direct compile path"
        );
        return compile_app_with_cache_uncached_path_shared(
            source_root,
            app_id,
            &cache_key,
            options,
            components_root,
        );
    };
    if !is_leader {
        return match wait_for_compile_inflight(&inflight) {
            Ok(compiled) => Ok(CompileWithCacheOutcomeShared {
                compiled,
                cache_hit: true,
                artifact_cache_hit: false,
                compile_revision: compile_revision(source_root, app_id, &options, components_root)
                    .token,
                revision_scope: "singleflight_wait".to_string(),
                cache_validation: "singleflight_wait".to_string(),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                artifact_load_ms: 0,
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
            Err(message) => Err(CompileWithCacheFailure {
                error: anyhow::anyhow!(message),
                revision_scope: "singleflight_wait".to_string(),
                cache_validation: "singleflight_wait".to_string(),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
        };
    }
    let outcome = compile_app_with_cache_uncached_path_shared(
        source_root,
        app_id,
        &cache_key,
        options,
        components_root,
    );
    match &outcome {
        Ok(value) => {
            clear_compile_failure(&cache_key);
            finish_compile_inflight(&cache_key, &inflight, Ok(value.compiled.clone()))
        }
        Err(error) => {
            record_compile_failure(&cache_key);
            finish_compile_inflight(&cache_key, &inflight, Err(error.error.to_string()))
        }
    }
    outcome
}

pub(super) fn compile_app_with_cache_uncached_path_shared(
    source_root: &Path,
    app_id: &str,
    cache_key: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcomeShared, CompileWithCacheFailure> {
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    let mut had_cache_entry = false;
    if let Ok(cache) = compile_cache().read() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(cache_key) {
            had_cache_entry = true;
            if let Some(hit) =
                validate_cached_entry(source_root, app_id, entry, components_root, &options)
            {
                ensure_compiled_app_artifact_alias(
                    source_root,
                    app_id,
                    &options,
                    components_root,
                    entry.compiled.as_ref(),
                );
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcomeShared {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    artifact_cache_hit: false,
                    compile_revision: hit.compile_revision,
                    revision_scope: hit.revision_scope,
                    cache_validation: hit.cache_validation,
                    cache_lookup_ms,
                    artifact_load_ms: 0,
                    compile_cache_lock_wait_ms,
                    compile_ms: 0,
                });
            }
        }
        cache_lookup_ms = elapsed_ms(lookup_started);
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during lookup; fallback to direct compile"
        );
        cache_lookup_ms = elapsed_ms(lookup_lock_started);
    }
    if let Some((artifact_hit, artifact_load_ms)) =
        maybe_load_compiled_app_artifact(source_root, app_id, &options, components_root)
    {
        ensure_compiled_app_artifact_alias(
            source_root,
            app_id,
            &options,
            components_root,
            artifact_hit.compiled.as_ref(),
        );
        return Ok(CompileWithCacheOutcomeShared {
            compiled: artifact_hit.compiled,
            cache_hit: true,
            artifact_cache_hit: true,
            compile_revision: artifact_hit.compile_revision,
            revision_scope: artifact_hit.revision_scope,
            cache_validation: artifact_hit.cache_validation,
            cache_lookup_ms,
            artifact_load_ms,
            compile_cache_lock_wait_ms,
            compile_ms: 0,
        });
    }
    let alias_options = options.clone();
    let compile_started = Instant::now();
    let (compiled, revision_stamp) = if had_cache_entry {
        let revision_stamp = compile_revision(source_root, app_id, &options, components_root);
        let compiled = match compile_app_with_options(source_root, app_id, options) {
            Ok(compiled) => compiled,
            Err(error) => {
                return Err(CompileWithCacheFailure {
                    error,
                    revision_scope: revision_stamp.scope.to_string(),
                    cache_validation: "miss".to_string(),
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: elapsed_ms(compile_started),
                });
            }
        };
        (compiled, revision_stamp)
    } else {
        match compile_app_with_options_and_revision(source_root, app_id, options) {
            Ok(artifacts) => (
                artifacts.compiled,
                revision::CompileRevisionStamp {
                    token: artifacts.revision_plan.token,
                    scope: "focused_graph",
                    watched_files: artifacts.revision_plan.watched_files,
                    components_revision: artifacts.revision_plan.components_revision,
                },
            ),
            Err(error) => {
                return Err(CompileWithCacheFailure {
                    error,
                    revision_scope: "miss".to_string(),
                    cache_validation: "miss".to_string(),
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: elapsed_ms(compile_started),
                });
            }
        }
    };
    let compile_ms = elapsed_ms(compile_started);
    let compiled = Arc::new(compiled);
    let write_lock_started = Instant::now();
    store_compile_cache_entry(
        cache_key,
        source_root,
        app_id,
        &alias_options,
        &revision_stamp.token,
        &revision_stamp.watched_files,
        revision_stamp.components_revision,
        compiled.clone(),
    );
    compile_cache_lock_wait_ms += elapsed_ms(write_lock_started);
    maybe_write_compiled_app_artifact(
        source_root,
        app_id,
        &alias_options,
        &revision_stamp,
        &compiled,
    );
    Ok(CompileWithCacheOutcomeShared {
        compiled,
        cache_hit: false,
        artifact_cache_hit: false,
        compile_revision: revision_stamp.token.clone(),
        revision_scope: revision_stamp.scope.to_string(),
        cache_validation: "miss".to_string(),
        cache_lookup_ms,
        artifact_load_ms: 0,
        compile_cache_lock_wait_ms,
        compile_ms,
    })
}

fn evict_compile_cache_entries_for_write(
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

fn validate_cached_entry(
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
    format!(
        "{}#{app_id}|v5|gen={COMPILE_SEMANTICS_GENERATION}|scene={}|focus={}",
        normalize_path(source_root),
        options.scene.as_deref().unwrap_or(""),
        options.preview_target.as_deref().unwrap_or("")
    )
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(super) fn watched_files_are_fresh(
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
        let path = app_root.join(&watched.rel_path);
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

pub(super) fn default_scene_alias_keys(
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
    let default_scene = compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .map(|route| route.scene_id.trim())
        .filter(|scene| !scene.is_empty());
    let (Some(active_scene), Some(default_scene)) = (active_scene, default_scene) else {
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
        },
    );
    let explicit_default_key = compile_cache_key(
        source_root,
        app_id,
        &CompileOptions {
            scene: Some(default_scene.to_string()),
            preview_target: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::CompiledApp;

    fn compiled_with_scene(active_scene: Option<&str>) -> CompiledApp {
        CompiledApp {
            app_id: "zhifa".to_string(),
            title: "zhifa".to_string(),
            app_root: "/tmp/zhifa".to_string(),
            scene_routes: Vec::new(),
            active_scene: active_scene.map(str::to_string),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        }
    }

    #[test]
    fn artifact_matches_compile_scene_request_requires_bound_scene() {
        let options = CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        };
        assert!(artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(Some("home"))
        ));
        assert!(!artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(None)
        ));
        assert!(!artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(Some("other"))
        ));
    }

    #[test]
    fn artifact_matches_compile_scene_request_allows_full_app_lookup() {
        let options = CompileOptions::default();
        assert!(artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(None)
        ));
    }
}
