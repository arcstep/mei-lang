use std::sync::atomic::{AtomicU64, Ordering};

static ARTIFACT_TMP_NONCE: AtomicU64 = AtomicU64::new(0);

pub fn take_metric_response_index_stats() -> MetricResponseIndexStats {
    MetricResponseIndexStats::default()
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
    mei_lang_kernel::resolve_app_eval_cache_root(app_root)
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
    let payload = serde_json::to_string_pretty(artifact)?;
    let tmp = path.with_extension(format!(
        "tmp-{}-{}-{}",
        std::process::id(),
        now_epoch_ms(),
        ARTIFACT_TMP_NONCE.fetch_add(1, Ordering::Relaxed),
    ));
    fs::write(&tmp, &payload)
        .with_context(|| format!("write result artifact tmp {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "rename result artifact {} -> {}",
            tmp.display(),
            path.display()
        )
    })?;
    record_artifact_write(payload.len() as u64);
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
    if let Some(existing) = read_json_artifact_lenient::<PersistedMetricResponseResultArtifact>(
        &path,
        "metric-response",
    )? {
        if existing.schema_version == METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION
            && existing.response_cache_key == response_cache_key
        {
            let already_covers = if complete {
                existing.complete
                    && covered_metric_ids
                        .iter()
                        .all(|id| existing.covered_metric_ids.contains(id))
            } else {
                covered_metric_ids
                    .iter()
                    .all(|id| existing.covered_metric_ids.contains(id))
            };
            if already_covers && (!complete || existing.complete) {
                record_response_store_skipped();
                return Ok(());
            }
            // Immutable complete artifact: never merge-rewrite; only replace when incomplete.
            if existing.complete && complete {
                record_response_store_skipped();
                return Ok(());
            }
        }
    }

    let persisted = PersistedMetricResponseResultArtifact {
        schema_version: METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
        response_cache_key: response_cache_key.to_string(),
        total_rows,
        metrics_map: metrics_map.clone(),
        covered_metric_ids: covered_metric_ids.clone(),
        complete,
        generated_at_ms: now_epoch_ms(),
        slot_revision: None,
    };
    write_json_artifact(&path, &persisted)?;
    record_response_store_atomic();
    Ok(())
}

