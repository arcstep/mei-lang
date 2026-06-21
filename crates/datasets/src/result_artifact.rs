use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::{Deserialize, Serialize};

use crate::metric_response_cache::{
    metric_response_prebuild_dataset_key, prebuild_metric_response_key_matches_dataset_query,
};
use crate::types::DatasetQueryOptions;
use crate::DatasetQueryResult;

const METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-response-result-artifact-v1";
const METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-dataframe-result-artifact-v1";

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
    app_root.join(".mei").join("eval-artifacts").join("results")
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

fn read_json_artifact<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let artifact = serde_json::from_str::<T>(
        &fs::read_to_string(path)
            .with_context(|| format!("read result artifact {}", path.display()))?,
    )
    .with_context(|| format!("parse result artifact {}", path.display()))?;
    Ok(Some(artifact))
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
    let Some(artifact) = read_json_artifact::<PersistedMetricResponseResultArtifact>(&path)? else {
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
    if let Some(existing) = read_json_artifact::<PersistedMetricResponseResultArtifact>(&path)? {
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
    };
    write_json_artifact(&path, &persisted)?;
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
                        response_cache_key: dataset_key,
                        ..persisted.clone()
                    },
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

fn prebuild_metric_response_index() -> &'static Mutex<Option<PrebuildMetricResponseIndex>> {
    static INDEX: OnceLock<Mutex<Option<PrebuildMetricResponseIndex>>> = OnceLock::new();
    INDEX.get_or_init(|| Mutex::new(None))
}

fn ensure_prebuild_metric_response_index(app_root: &Path) -> Result<()> {
    let Ok(mut guard) = prebuild_metric_response_index().lock() else {
        return Ok(());
    };
    if guard
        .as_ref()
        .is_some_and(|index| index.app_root == app_root)
    {
        return Ok(());
    }
    let mut entries = Vec::new();
    let dir = eval_result_artifact_root(app_root).join("metric-response");
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(artifact) =
                read_json_artifact::<PersistedMetricResponseResultArtifact>(&path)?
            else {
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
    *guard = Some(PrebuildMetricResponseIndex {
        app_root: app_root.to_path_buf(),
        entries,
    });
    Ok(())
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
    let mut best: Option<(String, u64, bool, usize)> = None;
    for entry in &index.entries {
        if !prebuild_metric_response_key_matches_dataset_query(
            entry.response_cache_key.as_str(),
            app_id,
            dataset_id,
            query,
        ) {
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
    let Some(artifact) = read_json_artifact::<PersistedMetricDataframeResultArtifact>(&path)? else {
        return Ok(None);
    };
    if artifact.schema_version != METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION
        || artifact.response_cache_key != response_cache_key
    {
        return Ok(None);
    }
    Ok(Some((artifact.result, started.elapsed().as_millis() as u64)))
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
    fn dataset_fallback_finds_zhifa_supervision_world_metrics() {
        let app_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../workspaces/ws-spbjw/zhifa");
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
