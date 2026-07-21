fn metric_dataframe_result_cache() -> &'static Cache<String, Arc<DatasetQueryResult>> {
    static CACHE: OnceLock<Cache<String, Arc<DatasetQueryResult>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(METRIC_DATAFRAME_CACHE_MAX_BYTES)
            .weigher(|_key: &String, result: &Arc<DatasetQueryResult>| {
                serde_json::to_vec(result.as_ref())
                    .map(|bytes| bytes.len().clamp(1, u32::MAX as usize) as u32)
                    .unwrap_or(128)
            })
            .time_to_live(Duration::from_millis(METRIC_DATAFRAME_CACHE_TTL_MS))
            .build()
    })
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
        facet_columns: options.facet_columns.clone(),
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
        "{}|compile={}|{}|scene={}|target={}|{}|{}|search={}|filters={}|group={}|time_range={}|filter_intents={}|facets={}",
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
        serialize_cache_value(&options.facet_columns),
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

fn take_cached_metric_dataframe_result(key: &str) -> Option<DatasetQueryResult> {
    metric_dataframe_result_cache()
        .get(key)
        .map(|result| result.as_ref().clone())
}

fn store_cached_metric_dataframe_result(key: String, result: &DatasetQueryResult) {
    if (result.rows.is_empty() && result.total == 0) || key.contains("|full=true|") {
        return;
    }
    let bytes = serde_json::to_vec(result)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    if bytes > METRIC_DATAFRAME_MAX_VALUE_BYTES {
        return;
    };
    metric_dataframe_result_cache().insert(key, Arc::new(result.clone()));
}

pub(crate) fn clear_metric_dataframe_result_cache() -> usize {
    let cache = metric_dataframe_result_cache();
    let removed = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
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

