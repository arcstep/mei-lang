use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use anyhow;
use mei_lang_kernel::{
    AnalysisGraph, CompileWatchedFile, CompiledApp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct CachedCompiledApp {
    pub(crate) compile_revision: String,
    pub(crate) watched_files: Vec<CompileWatchedFile>,
    pub(crate) components_revision: u128,
    pub(crate) compiled: Arc<CompiledApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct DatasetRuntimePayload {
    #[serde(default)]
    pub(crate) runtime_metric_defs: BTreeMap<String, Value>,
    #[serde(default)]
    pub(crate) runtime_analysis_graph: AnalysisGraph,
    #[serde(default)]
    pub(crate) runtime_analysis_contracts: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AssemblyInputDiskRecord {
    pub(crate) kind: String,
    pub(crate) key: String,
    pub(crate) revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompiledAppDiskArtifact {
    pub(crate) schema_version: String,
    pub(crate) compile_revision: String,
    pub(crate) revision_scope: String,
    pub(crate) compiled: CompiledApp,
    /// `DatasetView::runtime_*` fields are `serde(skip)` on the public model, so they must be
    /// stored alongside the compiled app for artifact reload to support runtime metric eval.
    #[serde(default)]
    pub(crate) dataset_runtime_payloads: BTreeMap<String, DatasetRuntimePayload>,
    /// MCG input node revisions for PageInstance derivation (see doc 80).
    #[serde(default, rename = "assemblyInputs")]
    pub(crate) assembly_inputs: Vec<AssemblyInputDiskRecord>,
    /// When true, `compiled` omits inline dataset rows and compile-time metric snapshots.
    #[serde(default, rename = "accessSlim")]
    pub(crate) access_slim: bool,
}

pub(crate) const COMPILED_APP_ARTIFACT_SCHEMA_VERSION: &str = "mei-compiled-app-artifact-v3";
pub(crate) const COMPILED_APP_ARTIFACT_SLIM_SCHEMA_VERSION: &str = "mei-compiled-app-artifact-v4";
pub(crate) const COMPILED_APP_ARTIFACT_KIND: &str = "compiled_app";
pub(crate) const COMPILED_APP_ARTIFACT_NAME: &str = "compiled_app";

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
    pub(crate) fn into_owned(self) -> CompileWithCacheOutcome {
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
    pub(crate) fn into_owned(self) -> PeekCompileCacheHit {
        PeekCompileCacheHit {
            compiled: (*self.compiled).clone(),
            compile_revision: self.compile_revision,
            revision_scope: self.revision_scope,
            cache_validation: self.cache_validation,
        }
    }
}

pub(crate) fn compile_cache() -> &'static RwLock<HashMap<String, CachedCompiledApp>> {
    static COMPILE_CACHE: OnceLock<RwLock<HashMap<String, CachedCompiledApp>>> = OnceLock::new();
    COMPILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn compile_failure_latch() -> &'static StdMutex<HashMap<String, Instant>> {
    static COMPILE_FAILURE_LATCH: OnceLock<StdMutex<HashMap<String, Instant>>> = OnceLock::new();
    COMPILE_FAILURE_LATCH.get_or_init(|| StdMutex::new(HashMap::new()))
}

pub(crate) const COMPILE_FAILURE_LATCH_TTL: Duration = Duration::from_secs(45);

pub(crate) fn compile_cache_max_entries() -> usize {
    static MAX_ENTRIES: OnceLock<usize> = OnceLock::new();
    *MAX_ENTRIES.get_or_init(|| {
        std::env::var("MEI_COMPILE_CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10240)
    })
}
