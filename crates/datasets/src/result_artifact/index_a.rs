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

