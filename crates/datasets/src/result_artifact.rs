use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::{Deserialize, Serialize};

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
    write_json_artifact(
        &metric_response_result_artifact_path(app_root, response_cache_key),
        &PersistedMetricResponseResultArtifact {
            schema_version: METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
            response_cache_key: response_cache_key.to_string(),
            total_rows,
            metrics_map: metrics_map.clone(),
            covered_metric_ids: covered_metric_ids.clone(),
            complete,
            generated_at_ms: now_epoch_ms(),
        },
    )
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
    if result.rows.is_empty() && result.total == 0 {
        return Ok(());
    }
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
