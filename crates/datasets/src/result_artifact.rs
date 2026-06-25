use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::metric_response_cache::{
    metric_response_prebuild_dataset_key, prebuild_metric_response_key_matches_dataset_query,
};
use crate::types::DatasetQueryOptions;
use crate::util::read_json_artifact_lenient;
use crate::DatasetQueryResult;

const METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-response-result-artifact-v1";
const METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-dataframe-result-artifact-v1";
const METRIC_RESPONSE_INDEX_SCHEMA_VERSION: &str = "mei-metric-response-index-v1";

#[derive(Debug, Clone, Copy, Default)]
pub struct MetricResponseIndexStats {
    pub load_ms: u64,
    pub entry_count: usize,
    pub rebuilt: bool,
}

thread_local! {
    static LAST_METRIC_RESPONSE_INDEX_STATS: Cell<MetricResponseIndexStats> =
        Cell::new(MetricResponseIndexStats::default());
}

pub fn take_metric_response_index_stats() -> MetricResponseIndexStats {
    LAST_METRIC_RESPONSE_INDEX_STATS.with(|cell| {
        let stats = cell.get();
        cell.set(MetricResponseIndexStats::default());
        stats
    })
}

fn record_metric_response_index_stats(stats: MetricResponseIndexStats) {
    LAST_METRIC_RESPONSE_INDEX_STATS.with(|cell| cell.set(stats));
}

#[derive(Debug, Clone)]
pub struct LoadedMetricResponseArtifact {
    pub total_rows: usize,
    pub metrics_map: BTreeMap<String, MetricContract>,
    pub covered_metric_ids: BTreeSet<String>,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetricResponseResultArtifact {
    schema_version: String,
    response_cache_key: String,
    total_rows: usize,
    metrics_map: BTreeMap<String, MetricContract>,
    covered_metric_ids: BTreeSet<String>,
    complete: bool,
    generated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "slotRevision")]
    slot_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetricDataframeResultArtifact {
    schema_version: String,
    response_cache_key: String,
    result: DatasetQueryResult,
    generated_at_ms: u64,
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_key(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn eval_result_artifact_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results")
}

fn metric_response_result_artifact_path(app_root: &Path, response_cache_key: &str) -> PathBuf {
    eval_result_artifact_root(app_root)
        .join("metric-response")
        .join(format!("{}.json", hash_key(response_cache_key)))
}

fn metric_dataframe_result_artifact_path(app_root: &Path, response_cache_key: &str) -> PathBuf {
    eval_result_artifact_root(app_root)
        .join("metric-dataframe")
        .join(format!("{}.json", hash_key(response_cache_key)))
}

fn write_json_artifact<T: Serialize>(path: &Path, artifact: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create result artifact dir {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(artifact)?)
        .with_context(|| format!("write result artifact {}", path.display()))?;
    Ok(())
}

fn write_json_artifact_atomic<T: Serialize>(path: &Path, artifact: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create result artifact dir {}", parent.display()))?;
    }
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_string_pretty(artifact)?)
        .with_context(|| format!("write temp artifact {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| format!("rename artifact {}", path.display()))?;
    Ok(())
}

pub fn default_result_artifact_scope(
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
) -> bool {
    query_state.filters.is_empty()
        && query_state
            .search
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && query_state.group.is_empty()
        && query_state.time_range.is_none()
        && filter_intents.is_empty()
}

pub fn load_metric_response_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
) -> Result<Option<(LoadedMetricResponseArtifact, u64)>> {
    let started = Instant::now();
    let path = metric_response_result_artifact_path(app_root, response_cache_key);
    let Some(artifact) = read_json_artifact_lenient::<PersistedMetricResponseResultArtifact>(
        &path,
        "metric-response",
    )?
    else {
        return Ok(None);
    };
    if artifact.schema_version != METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION
        || artifact.response_cache_key != response_cache_key
    {
        return Ok(None);
    }
    Ok(Some((
        LoadedMetricResponseArtifact {
            total_rows: artifact.total_rows,
            metrics_map: artifact.metrics_map,
            covered_metric_ids: artifact.covered_metric_ids,
            complete: artifact.complete,
        },
        started.elapsed().as_millis() as u64,
    )))
}

pub fn metric_response_result_artifact_exists(app_root: &Path, response_cache_key: &str) -> bool {
    let path = metric_response_result_artifact_path(app_root, response_cache_key);
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub fn store_metric_response_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) -> Result<()> {
    let path = metric_response_result_artifact_path(app_root, response_cache_key);
    let mut merged_total_rows = total_rows;
    let mut merged_metrics_map = metrics_map.clone();
    let mut merged_covered_metric_ids = covered_metric_ids.clone();
    let mut merged_complete = complete;
    if let Some(existing) = read_json_artifact_lenient::<PersistedMetricResponseResultArtifact>(
        &path,
        "metric-response",
    )? {
        if existing.schema_version == METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION
            && existing.response_cache_key == response_cache_key
        {
            merged_total_rows = existing.total_rows.max(total_rows);
            let mut existing_metrics_map = existing.metrics_map;
            existing_metrics_map.extend(merged_metrics_map);
            merged_metrics_map = existing_metrics_map;
            merged_covered_metric_ids.extend(existing.covered_metric_ids);
            merged_complete |= existing.complete;
        }
    }
    let persisted = PersistedMetricResponseResultArtifact {
        schema_version: METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
        response_cache_key: response_cache_key.to_string(),
        total_rows: merged_total_rows,
        metrics_map: merged_metrics_map,
        covered_metric_ids: merged_covered_metric_ids,
        complete: merged_complete,
        generated_at_ms: now_epoch_ms(),
        slot_revision: None,
    };
    write_json_artifact(&path, &persisted)?;
    upsert_metric_response_index_entry(
        app_root,
        response_cache_key,
        persisted.generated_at_ms,
        persisted.complete,
        &persisted.covered_metric_ids,
    )?;
    if response_cache_key.starts_with("prebuild|response|")
        && response_cache_key.contains("|dependency=")
    {
        if let Some((app_id, dataset_id, query)) =
            parse_prebuild_metric_response_key(response_cache_key)
        {
            let dataset_key =
                metric_response_prebuild_dataset_key(app_id.as_str(), dataset_id.as_str(), &query);
            if dataset_key != response_cache_key {
                write_json_artifact(
                    &metric_response_result_artifact_path(app_root, dataset_key.as_str()),
                    &PersistedMetricResponseResultArtifact {
                        response_cache_key: dataset_key.clone(),
                        ..persisted.clone()
                    },
                )?;
                upsert_metric_response_index_entry(
                    app_root,
                    dataset_key.as_str(),
                    persisted.generated_at_ms,
                    persisted.complete,
                    &persisted.covered_metric_ids,
                )?;
            }
        }
    }
    Ok(())
}

fn parse_prebuild_metric_response_key(
    response_cache_key: &str,
) -> Option<(String, String, DatasetQueryOptions)> {
    let rest = response_cache_key.strip_prefix("prebuild|response|")?;
    let (app_part, rest) = rest.split_once('|')?;
    let app_id = app_part.strip_prefix("app=")?.to_string();
    let (dataset_part, rest) = rest.split_once('|')?;
    let dataset_id = dataset_part.strip_prefix("dataset=")?.to_string();
    let rest = rest.strip_prefix("dependency=")?;
    let (_, query_tail) = rest.split_once('|')?;
    let mut query = DatasetQueryOptions::default();
    for segment in query_tail.split('|') {
        if let Some(value) = segment.strip_prefix("search=") {
            query.search = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        } else if let Some(value) = segment.strip_prefix("filters=") {
            query.filters = serde_json::from_str(value).unwrap_or_default();
        } else if let Some(value) = segment.strip_prefix("group=") {
            query.group = serde_json::from_str(value).unwrap_or_default();
        } else if let Some(value) = segment.strip_prefix("time_range=") {
            query.time_range = serde_json::from_str(value).ok();
        }
    }
    Some((app_id, dataset_id, query))
}

#[derive(Clone)]
struct PrebuildMetricResponseIndexEntry {
    response_cache_key: String,
    generated_at_ms: u64,
    complete: bool,
    covered_metric_ids: BTreeSet<String>,
}

struct PrebuildMetricResponseIndex {
    app_root: PathBuf,
    entries: Vec<PrebuildMetricResponseIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetricResponseIndexSidecarEntry {
    response_cache_key: String,
    artifact_basename: String,
    generated_at_ms: u64,
    complete: bool,
    covered_metric_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetricResponseIndexSidecar {
    schema_version: String,
    generated_at_ms: u64,
    fingerprint: String,
    entries: Vec<PersistedMetricResponseIndexSidecarEntry>,
}

#[derive(Deserialize)]
struct PersistedMetricResponseIndexSource {
    schema_version: String,
    response_cache_key: String,
    #[serde(default, rename = "metrics_map")]
    _metrics_map: IgnoredAny,
    #[serde(default)]
    covered_metric_ids: BTreeSet<String>,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    generated_at_ms: u64,
}

fn prebuild_metric_response_index() -> &'static Mutex<Option<PrebuildMetricResponseIndex>> {
    static INDEX: OnceLock<Mutex<Option<PrebuildMetricResponseIndex>>> = OnceLock::new();
    INDEX.get_or_init(|| Mutex::new(None))
}

fn metric_response_index_path(app_root: &Path) -> PathBuf {
    eval_result_artifact_root(app_root).join("metric-response-index.json")
}

fn metric_response_artifact_dir(app_root: &Path) -> PathBuf {
    eval_result_artifact_root(app_root).join("metric-response")
}

fn hash_file_metadata(
    path: &Path,
    hasher: &mut std::collections::hash_map::DefaultHasher,
) -> Result<()> {
    let meta = fs::metadata(path)?;
    meta.len().hash(hasher);
    if let Ok(modified) = meta.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            duration.as_secs().hash(hasher);
            duration.subsec_nanos().hash(hasher);
        }
    }
    Ok(())
}

fn compute_metric_response_dir_fingerprint(dir: &Path) -> Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if !dir.is_dir() {
        0usize.hash(&mut hasher);
        return Ok(format!("{:016x}", hasher.finish()));
    }
    let mut files = fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort_by_key(|entry| entry.file_name());
    files.len().hash(&mut hasher);
    for entry in files {
        hash_file_metadata(entry.path().as_path(), &mut hasher)?;
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn sidecar_entry_from_memory(
    entry: &PrebuildMetricResponseIndexEntry,
) -> PersistedMetricResponseIndexSidecarEntry {
    PersistedMetricResponseIndexSidecarEntry {
        response_cache_key: entry.response_cache_key.clone(),
        artifact_basename: format!("{}.json", hash_key(entry.response_cache_key.as_str())),
        generated_at_ms: entry.generated_at_ms,
        complete: entry.complete,
        covered_metric_ids: entry.covered_metric_ids.clone(),
    }
}

fn index_from_sidecar(
    app_root: &Path,
    sidecar: PersistedMetricResponseIndexSidecar,
) -> PrebuildMetricResponseIndex {
    PrebuildMetricResponseIndex {
        app_root: app_root.to_path_buf(),
        entries: sidecar
            .entries
            .into_iter()
            .map(|entry| PrebuildMetricResponseIndexEntry {
                response_cache_key: entry.response_cache_key,
                generated_at_ms: entry.generated_at_ms,
                complete: entry.complete,
                covered_metric_ids: entry.covered_metric_ids,
            })
            .collect(),
    }
}

fn save_metric_response_index_sidecar(
    app_root: &Path,
    index: &PrebuildMetricResponseIndex,
) -> Result<()> {
    let dir = metric_response_artifact_dir(app_root);
    let fingerprint = compute_metric_response_dir_fingerprint(dir.as_path())?;
    let sidecar = PersistedMetricResponseIndexSidecar {
        schema_version: METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string(),
        generated_at_ms: now_epoch_ms(),
        fingerprint,
        entries: index
            .entries
            .iter()
            .map(sidecar_entry_from_memory)
            .collect(),
    };
    write_json_artifact_atomic(metric_response_index_path(app_root).as_path(), &sidecar)
}

fn load_metric_response_index_from_sidecar(
    app_root: &Path,
    verify_fingerprint: bool,
) -> Result<Option<PrebuildMetricResponseIndex>> {
    let path = metric_response_index_path(app_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read metric response index {}", path.display()))?;
    let sidecar = serde_json::from_str::<PersistedMetricResponseIndexSidecar>(&raw)
        .with_context(|| format!("parse metric response index {}", path.display()))?;
    if sidecar.schema_version != METRIC_RESPONSE_INDEX_SCHEMA_VERSION {
        return Ok(None);
    }
    let dir = metric_response_artifact_dir(app_root);
    let fingerprint = compute_metric_response_dir_fingerprint(dir.as_path())?;
    if sidecar.fingerprint != fingerprint {
        if verify_fingerprint {
            tracing::warn!(
                app_root = %app_root.display(),
                "metric response index fingerprint mismatch; rebuilding sidecar"
            );
            return Ok(None);
        }
        tracing::debug!(
            app_root = %app_root.display(),
            "metric response index fingerprint mismatch; using sidecar entries on request path"
        );
    }
    Ok(Some(index_from_sidecar(app_root, sidecar)))
}

fn read_prebuild_metric_response_index_source(
    path: &Path,
) -> Result<Option<PersistedMetricResponseIndexSource>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read metric response artifact {}", path.display()))?;
    let artifact = serde_json::from_str::<PersistedMetricResponseIndexSource>(&raw)
        .with_context(|| format!("parse metric response artifact metadata {}", path.display()))?;
    Ok(Some(artifact))
}

fn rebuild_prebuild_metric_response_index_from_artifacts(
    app_root: &Path,
) -> Result<PrebuildMetricResponseIndex> {
    let mut entries = Vec::new();
    let dir = metric_response_artifact_dir(app_root);
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(artifact) = read_prebuild_metric_response_index_source(path.as_path())? else {
                continue;
            };
            if artifact.schema_version != METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION
                || !artifact
                    .response_cache_key
                    .starts_with("prebuild|response|")
            {
                continue;
            }
            entries.push(PrebuildMetricResponseIndexEntry {
                response_cache_key: artifact.response_cache_key,
                generated_at_ms: artifact.generated_at_ms,
                complete: artifact.complete,
                covered_metric_ids: artifact.covered_metric_ids,
            });
        }
    }
    Ok(PrebuildMetricResponseIndex {
        app_root: app_root.to_path_buf(),
        entries,
    })
}

fn install_prebuild_metric_response_index(index: PrebuildMetricResponseIndex) -> usize {
    let entry_count = index.entries.len();
    if let Ok(mut guard) = prebuild_metric_response_index().lock() {
        *guard = Some(index);
    }
    entry_count
}

fn try_load_prebuild_metric_response_index_from_sidecar(
    app_root: &Path,
    verify_fingerprint: bool,
) -> Result<Option<PrebuildMetricResponseIndex>> {
    load_metric_response_index_from_sidecar(app_root, verify_fingerprint)
}

fn rebuild_prebuild_metric_response_index(app_root: &Path) -> Result<PrebuildMetricResponseIndex> {
    let index = rebuild_prebuild_metric_response_index_from_artifacts(app_root)?;
    save_metric_response_index_sidecar(app_root, &index)?;
    Ok(index)
}

fn upsert_metric_response_index_entry(
    app_root: &Path,
    response_cache_key: &str,
    generated_at_ms: u64,
    complete: bool,
    covered_metric_ids: &BTreeSet<String>,
) -> Result<()> {
    if !response_cache_key.starts_with("prebuild|response|") {
        return Ok(());
    }
    let memory_entry = PrebuildMetricResponseIndexEntry {
        response_cache_key: response_cache_key.to_string(),
        generated_at_ms,
        complete,
        covered_metric_ids: covered_metric_ids.clone(),
    };
    if let Ok(mut guard) = prebuild_metric_response_index().lock() {
        if let Some(index) = guard.as_mut() {
            if index.app_root == app_root {
                if let Some(existing) = index
                    .entries
                    .iter_mut()
                    .find(|entry| entry.response_cache_key == response_cache_key)
                {
                    *existing = memory_entry.clone();
                } else {
                    index.entries.push(memory_entry.clone());
                }
            }
        }
    }

    let path = metric_response_index_path(app_root);
    let dir = metric_response_artifact_dir(app_root);
    let fingerprint = compute_metric_response_dir_fingerprint(dir.as_path())?;
    let mut sidecar = if path.is_file() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str::<PersistedMetricResponseIndexSidecar>(&raw).unwrap_or_else(|_| {
            PersistedMetricResponseIndexSidecar {
                schema_version: METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string(),
                generated_at_ms: now_epoch_ms(),
                fingerprint: fingerprint.clone(),
                entries: Vec::new(),
            }
        })
    } else {
        PersistedMetricResponseIndexSidecar {
            schema_version: METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string(),
            generated_at_ms: now_epoch_ms(),
            fingerprint: fingerprint.clone(),
            entries: Vec::new(),
        }
    };
    sidecar.schema_version = METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string();
    sidecar.generated_at_ms = now_epoch_ms();
    sidecar.fingerprint = fingerprint;
    let sidecar_entry = sidecar_entry_from_memory(&memory_entry);
    if let Some(existing) = sidecar
        .entries
        .iter_mut()
        .find(|entry| entry.response_cache_key == response_cache_key)
    {
        *existing = sidecar_entry;
    } else {
        sidecar.entries.push(sidecar_entry);
    }
    write_json_artifact_atomic(path.as_path(), &sidecar)
}

pub fn invalidate_prebuild_metric_response_index(app_root: Option<&Path>) {
    let Ok(mut guard) = prebuild_metric_response_index().lock() else {
        return;
    };
    match app_root {
        Some(root) => {
            if guard.as_ref().is_some_and(|index| index.app_root == root) {
                *guard = None;
            }
        }
        None => *guard = None,
    }
}

/// Startup / post-prebuild: load sidecar when possible; rebuild only when sidecar is absent.
pub fn preload_prebuild_metric_response_index(app_root: &Path) -> Result<MetricResponseIndexStats> {
    if let Ok(guard) = prebuild_metric_response_index().lock() {
        if guard
            .as_ref()
            .is_some_and(|index| index.app_root == app_root)
        {
            let stats = MetricResponseIndexStats {
                load_ms: 0,
                entry_count: guard.as_ref().map(|index| index.entries.len()).unwrap_or(0),
                rebuilt: false,
            };
            record_metric_response_index_stats(stats);
            return Ok(stats);
        }
    }

    let started = Instant::now();
    let rebuilt = if let Some(index) =
        try_load_prebuild_metric_response_index_from_sidecar(app_root, false)?
    {
        install_prebuild_metric_response_index(index);
        false
    } else {
        rebuild_and_install_prebuild_metric_response_index(app_root)?.rebuilt
    };
    let entry_count = prebuild_metric_response_index()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|index| index.entries.len()))
        .unwrap_or(0);
    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count,
        rebuilt,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

/// Prebuild finalize: always rescan artifacts once off the request hot path.
pub fn rebuild_and_install_prebuild_metric_response_index(
    app_root: &Path,
) -> Result<MetricResponseIndexStats> {
    let started = Instant::now();
    let index = rebuild_prebuild_metric_response_index(app_root)?;
    let entry_count = install_prebuild_metric_response_index(index);
    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count,
        rebuilt: true,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

fn ensure_prebuild_metric_response_index(app_root: &Path) -> Result<MetricResponseIndexStats> {
    if let Ok(guard) = prebuild_metric_response_index().lock() {
        if guard
            .as_ref()
            .is_some_and(|index| index.app_root == app_root)
        {
            let stats = MetricResponseIndexStats {
                load_ms: 0,
                entry_count: guard.as_ref().map(|index| index.entries.len()).unwrap_or(0),
                rebuilt: false,
            };
            record_metric_response_index_stats(stats);
            return Ok(stats);
        }
    }

    let started = Instant::now();
    if let Some(index) = try_load_prebuild_metric_response_index_from_sidecar(app_root, false)? {
        let entry_count = install_prebuild_metric_response_index(index);
        let stats = MetricResponseIndexStats {
            load_ms: started.elapsed().as_millis() as u64,
            entry_count,
            rebuilt: false,
        };
        record_metric_response_index_stats(stats);
        return Ok(stats);
    }

    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count: 0,
        rebuilt: false,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

pub fn prebuild_metric_response_index_covers_key(
    app_root: &Path,
    response_cache_key: &str,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Result<bool> {
    ensure_prebuild_metric_response_index(app_root)?;
    let Ok(guard) = prebuild_metric_response_index().lock() else {
        return Ok(false);
    };
    let Some(index) = guard.as_ref() else {
        return Ok(false);
    };
    Ok(index
        .entries
        .iter()
        .find(|entry| entry.response_cache_key == response_cache_key)
        .is_some_and(|entry| {
            if request_all_metrics {
                entry.complete
            } else {
                requested_metric_ids
                    .iter()
                    .all(|metric_id| entry.covered_metric_ids.contains(metric_id))
            }
        }))
}

pub fn load_prebuild_metric_response_artifact_dataset_fallback(
    app_root: &Path,
    app_id: &str,
    dataset_id: &str,
    query: &DatasetQueryOptions,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Result<Option<(String, LoadedMetricResponseArtifact, u64)>> {
    ensure_prebuild_metric_response_index(app_root)?;
    let Ok(guard) = prebuild_metric_response_index().lock() else {
        return Ok(None);
    };
    let Some(index) = guard.as_ref() else {
        return Ok(None);
    };
    let dataset_candidates = crate::metric_cache_key::dataset_resource_lookup_aliases(dataset_id);
    let mut best: Option<(String, u64, bool, usize)> = None;
    for entry in &index.entries {
        let dataset_matches = dataset_candidates.iter().any(|candidate| {
            prebuild_metric_response_key_matches_dataset_query(
                entry.response_cache_key.as_str(),
                app_id,
                candidate.as_str(),
                query,
            )
        });
        if !dataset_matches {
            continue;
        }
        let covers = if request_all_metrics {
            entry.complete
        } else {
            requested_metric_ids
                .iter()
                .all(|metric_id| entry.covered_metric_ids.contains(metric_id))
        };
        if !covers {
            continue;
        }
        let covered_count = if request_all_metrics {
            entry.covered_metric_ids.len()
        } else {
            requested_metric_ids.len()
        };
        let replace = best.as_ref().is_none_or(|(_, best_at, complete, count)| {
            entry.complete && !*complete
                || (entry.complete == *complete
                    && (entry.generated_at_ms > *best_at
                        || (entry.generated_at_ms == *best_at && covered_count > *count)))
        });
        if replace {
            best = Some((
                entry.response_cache_key.clone(),
                entry.generated_at_ms,
                entry.complete,
                covered_count,
            ));
        }
    }
    let Some((cache_key, _, _, _)) = best else {
        return Ok(None);
    };
    load_metric_response_result_artifact(app_root, cache_key.as_str())
        .map(|loaded| loaded.map(|(artifact, ms)| (cache_key, artifact, ms)))
}

pub fn load_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
) -> Result<Option<(DatasetQueryResult, u64)>> {
    let started = Instant::now();
    let path = metric_dataframe_result_artifact_path(app_root, response_cache_key);
    let Some(artifact) = read_json_artifact_lenient::<PersistedMetricDataframeResultArtifact>(
        &path,
        "metric-dataframe",
    )?
    else {
        return Ok(None);
    };
    if artifact.schema_version != METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION
        || artifact.response_cache_key != response_cache_key
    {
        return Ok(None);
    }
    Ok(Some((
        artifact.result,
        started.elapsed().as_millis() as u64,
    )))
}

pub fn metric_dataframe_result_artifact_exists(app_root: &Path, response_cache_key: &str) -> bool {
    let path = metric_dataframe_result_artifact_path(app_root, response_cache_key);
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub fn store_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
    result: &DatasetQueryResult,
) -> Result<()> {
    write_json_artifact(
        &metric_dataframe_result_artifact_path(app_root, response_cache_key),
        &PersistedMetricDataframeResultArtifact {
            schema_version: METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
            response_cache_key: response_cache_key.to_string(),
            result: result.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use crate::types::DatasetQueryOptions;
    use std::path::PathBuf;

    #[test]
    fn sidecar_roundtrip_preloads_memory_index() {
        let app_root =
            std::env::temp_dir().join(format!("mei-metric-index-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&app_root);
        fs::create_dir_all(&app_root).expect("create temp app root");
        let cache_key = "prebuild|response|app=demo|dataset=sample|dependency=dep|search=|filters={}|group=[]|time_range=null";
        store_metric_response_result_artifact(
            app_root.as_path(),
            cache_key,
            1,
            &BTreeMap::new(),
            &BTreeSet::from(["m1".to_string()]),
            true,
        )
        .expect("store artifact");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        let first = preload_prebuild_metric_response_index(app_root.as_path()).expect("preload");
        assert!(first.entry_count >= 1);
        assert!(metric_response_index_path(app_root.as_path()).is_file());
        let second =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("preload again");
        assert_eq!(second.load_ms, 0);
        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn lenient_sidecar_load_skips_rebuild_on_fingerprint_mismatch() {
        let app_root =
            std::env::temp_dir().join(format!("mei-metric-index-mismatch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&app_root);
        fs::create_dir_all(&app_root).expect("create temp app root");
        let cache_key = "prebuild|response|app=demo|dataset=sample|dependency=dep|search=|filters={}|group=[]|time_range=null";
        store_metric_response_result_artifact(
            app_root.as_path(),
            cache_key,
            1,
            &BTreeMap::new(),
            &BTreeSet::from(["m1".to_string()]),
            true,
        )
        .expect("store artifact");
        let _ = rebuild_and_install_prebuild_metric_response_index(app_root.as_path())
            .expect("initial rebuild");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        fs::write(
            metric_response_artifact_dir(app_root.as_path()).join("orphan.json"),
            "{}",
        )
        .expect("add orphan artifact without updating sidecar");
        let stats =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("lenient preload");
        assert!(
            stats.load_ms < 200,
            "fingerprint mismatch must not trigger full rebuild on preload, got {}ms",
            stats.load_ms
        );
        assert!(stats.entry_count >= 1);
        assert!(!stats.rebuilt);
        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn zhifa_sidecar_preload_is_fast_after_warm_sidecar() {
        let app_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-spbjw/zhifa");
        if !app_root.is_dir() {
            return;
        }
        let _ =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("initial preload");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        let stats =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("sidecar preload");
        assert!(
            stats.load_ms < 500,
            "expected fast sidecar preload for zhifa, got {}ms for {} entries",
            stats.load_ms,
            stats.entry_count
        );
    }

    #[test]
    fn dataset_fallback_finds_zhifa_supervision_world_metrics() {
        let app_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-spbjw/zhifa");
        if !app_root.is_dir() {
            return;
        }
        let query = DatasetQueryOptions::default();
        let requested = BTreeSet::from([
            "scenes/08-监督成效.mei::effectiveness_transfer_clue_count".to_string(),
        ]);
        let loaded = load_prebuild_metric_response_artifact_dataset_fallback(
            app_root.as_path(),
            "zhifa",
            "__world_metrics__::scenes/08-监督成效.mei::metrics",
            &query,
            &requested,
            false,
        )
        .expect("fallback load");
        assert!(
            loaded.is_some(),
            "expected prebuild artifact for 08-监督成效 world_metrics"
        );
    }
}
