fn metric_dataframe_result_cache() -> &'static Mutex<MetricDataframeCacheState> {
    static CACHE: OnceLock<Mutex<MetricDataframeCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MetricDataframeCacheState::default()))
}

fn metric_dataframe_materialized_cache() -> &'static Mutex<MetricDataframeMaterializedCacheState> {
    static CACHE: OnceLock<Mutex<MetricDataframeMaterializedCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MetricDataframeMaterializedCacheState::default()))
}

fn metric_dataframe_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_DATAFRAME_CACHE_TTL_MS)
}

fn metric_dataframe_materialized_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_DATAFRAME_MATERIALIZED_CACHE_TTL_MS)
}

/// 作用域过滤已在 base rowset 物化阶段应用；metric 输出列（如 pivot 的 month/2024/2025）
/// 不含原始维度字段，分页时不得再次套用 query_state.filters。
fn metric_output_pagination_options(options: &DatasetQueryOptions) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: options.page,
        page_size: options.page_size,
        collect_all: options.collect_all,
        sort: options.sort.clone(),
        column_state: options.column_state.clone(),
        summary: options.summary,
        search: None,
        filters: BTreeMap::new(),
        group: Vec::new(),
        time_range: None,
    }
}

fn hash_fingerprint(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn metric_dataframe_scope_cache_key(
    app_root: &Path,
    scene_id: Option<&str>,
    target: Option<&str>,
    dataset_id: &str,
    metric_id: &str,
    options: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
    filter_intents: &[FilterIntent],
) -> String {
    let group = serialize_cache_value(&options.group);
    let time_range = serialize_cache_value(&options.time_range);
    format!(
        "{}|compile={}|{}|scene={}|target={}|{}|{}|search={}|filters={}|group={}|time_range={}|filter_intents={}",
        app_root.display(),
        compile_revision,
        dependency_revision_key,
        scene_id.unwrap_or("").trim(),
        target.unwrap_or("").trim(),
        dataset_id,
        metric_id,
        options.search.as_deref().unwrap_or(""),
        serialize_cache_value(&options.filters),
        group,
        time_range,
        serde_json::to_string(filter_intents).unwrap_or_else(|_| "[]".to_string()),
    )
}

pub fn metric_dataframe_result_cache_key(
    app_root: &Path,
    scene_id: Option<&str>,
    target: Option<&str>,
    dataset_id: &str,
    metric_id: &str,
    options: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
    filter_intents: &[FilterIntent],
) -> String {
    let sort = serialize_cache_value(&options.sort);
    let column_state = serialize_cache_value(&options.column_state);
    format!(
        "{}|page={}|page_size={}|full={}|sort={}|column_state={}|summary={}",
        metric_dataframe_scope_cache_key(
            app_root,
            scene_id,
            target,
            dataset_id,
            metric_id,
            options,
            compile_revision,
            dependency_revision_key,
            filter_intents,
        ),
        options.page,
        options.page_size,
        options.collect_all,
        sort,
        column_state,
        options.summary
    )
}

impl MetricDataframeMaterializedCacheState {
    fn prune_if_due(&mut self, now: Instant) {
        if self.next_prune_at.is_some_and(|next| now < next) {
            return;
        }
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.next_prune_at =
            Some(now + Duration::from_millis(METRIC_DATAFRAME_CACHE_PRUNE_INTERVAL_MS));
    }
}

fn take_cached_metric_dataframe_materialized(key: &str) -> Option<MaterializedMetricDataframe> {
    let Ok(mut cache) = metric_dataframe_materialized_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    cache.entries.get(key).cloned()
}

fn store_cached_metric_dataframe_materialized(
    key: String,
    materialized: MaterializedMetricDataframe,
) {
    if materialized.rows.len() < MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE {
        return;
    }
    let Ok(mut cache) = metric_dataframe_materialized_cache().lock() else {
        return;
    };
    cache.prune_if_due(Instant::now());
    if cache.entries.len() >= MAX_METRIC_DATAFRAME_MATERIALIZED_ENTRIES {
        cache.entries.clear();
    }
    cache.entries.insert(key, materialized);
}

fn take_cached_metric_dataframe_result(key: &str) -> Option<DatasetQueryResult> {
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    cache.entries.get(key).map(|entry| entry.result.clone())
}

fn store_cached_metric_dataframe_result(key: String, result: &DatasetQueryResult) {
    if result.rows.is_empty() && result.total == 0 {
        return;
    }
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return;
    };
    cache.prune_if_due(Instant::now());
    cache.entries.insert(
        key,
        CachedMetricDataframeResult {
            expires_at: Instant::now() + metric_dataframe_cache_ttl(),
            result: result.clone(),
        },
    );
}

pub(crate) fn clear_metric_dataframe_result_cache() -> usize {
    let mut removed = metric_dataframe_result_cache()
        .lock()
        .ok()
        .map(|mut cache| {
            let count = cache.entries.len();
            cache.entries.clear();
            cache.next_prune_at = None;
            count
        })
        .unwrap_or(0);
    if let Ok(mut materialized) = metric_dataframe_materialized_cache().lock() {
        removed = removed.saturating_add(materialized.entries.len());
        materialized.entries.clear();
        materialized.next_prune_at = None;
    }
    removed
}

fn synthetic_scalar_rowset_parent(
    resource: &mei_lang_kernel::LoadedResource,
    resolved_metric_id: &str,
) -> Option<String> {
    if !resolved_metric_id.ends_with("::__scalar_rowset__") {
        return None;
    }
    let dataset = resource.dataset.as_ref()?;
    if dataset.runtime_metric_defs.contains_key(resolved_metric_id) {
        return None;
    }
    let parent_metric_id = resolved_metric_id.strip_suffix("::__scalar_rowset__")?;
    resolve_runtime_metric_def_key(
        resource.id.as_str(),
        parent_metric_id,
        &dataset.runtime_metric_defs,
    )
}

fn scalar_metric_to_rowset(metric: &MetricContract) -> (Vec<String>, Vec<Value>) {
    let columns = metric
        .schema
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    if let Value::Object(map) = &metric.value {
        if columns.is_empty() {
            let inferred = map.keys().cloned().collect::<Vec<_>>();
            return (inferred, vec![metric.value.clone()]);
        }
        return (columns, vec![metric.value.clone()]);
    }
    let column = columns
        .first()
        .cloned()
        .unwrap_or_else(|| "value".to_string());
    (
        vec![column.clone()],
        vec![serde_json::json!({ column: metric.value })],
    )
}

