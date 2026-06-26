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

