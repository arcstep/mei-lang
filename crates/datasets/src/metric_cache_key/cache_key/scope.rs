pub(crate) fn runtime_metric_eval_scope(
    binding_datasets: &[&DatasetView],
    base_dataset_id: &str,
    scene_id: &str,
    target: Option<&str>,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    query_state_override: Option<&QueryState>,
    filter_intents_override: &[FilterIntent],
    dependency_revision_key: &str,
) -> Result<RuntimeMetricEvalScope> {
    let normalized_filters = normalize_query_filters(filters);
    let query_state = query_state_from_request(&normalized_filters, search, query_state_override);
    let normalized_search = query_state.search.clone().unwrap_or_default();
    let filter_intents = filter_intents_from_request(&query_state, filter_intents_override);
    let dimension_bindings = if binding_datasets.is_empty() {
        dimension_bindings_from_query_state(&query_state)
    } else {
        validate_runtime_scope_bindings(&query_state, binding_datasets)?;
        dimension_bindings_from_query_state_for_datasets(&query_state, binding_datasets)
    };
    Ok(RuntimeMetricEvalScope {
        base_dataset_id: base_dataset_id.trim().to_string(),
        scene_id: scene_id.trim().to_string(),
        target: target.unwrap_or("").trim().to_string(),
        search: normalized_search,
        query_state,
        filter_intents,
        dimension_bindings,
        filters_fingerprint: serialize_cache_value(&normalized_filters),
        dependency_revision_key: dependency_revision_key.to_string(),
    })
}

fn validate_runtime_scope_bindings(state: &QueryState, datasets: &[&DatasetView]) -> Result<()> {
    use crate::metric_hydrate::{
        resolve_dataset_query_bindings_from_state, unresolved_filter_dimensions_for_datasets,
    };
    let unresolved = unresolved_filter_dimensions_for_datasets(state, datasets);
    if !unresolved.is_empty() {
        let dataset_ids = datasets
            .iter()
            .map(|dataset| dataset.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!(
            "runtime metric query requires resolvable filter bindings across datasets [{}]: {}",
            dataset_ids,
            unresolved.join(", ")
        ));
    }
    for dataset in datasets {
        let resolution = resolve_dataset_query_bindings_from_state(state, dataset);
        if let Some(dimension) = resolution.unresolved_time_range_dimension {
            return Err(anyhow!(
                "runtime metric query requires resolvable time_range.dimension binding for dataset `{}`: {}",
                dataset.id,
                dimension
            ));
        }
    }
    Ok(())
}

pub(crate) fn eval_node_cache_key(
    expr_fingerprint: &str,
    scope: &RuntimeMetricEvalScope,
) -> String {
    format!(
        "expr={}|dataset={}|scene={}|target={}|search={}|filters={}|filter_intents={}|dimension_bindings={}|group={}|time_range={}|deps={}",
        expr_fingerprint.trim(),
        scope.base_dataset_id.trim(),
        scope.scene_id.trim(),
        scope.target.trim(),
        scope.search.trim(),
        scope.filters_fingerprint.trim(),
        filter_intents_fingerprint(scope),
        dimension_bindings_fingerprint(scope),
        scope.query_state.group_identity_key(),
        scope.query_state.time_range_identity_key(),
        scope.dependency_revision_key.trim()
    )
}

fn dataset_source_fingerprint(app_root: &Path, dataset: &DatasetView) -> String {
    let kind = dataset.source.kind.trim();
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return format!(
            "{}|kind={}|path={}|sheet={}|header_row={}",
            dataset.id,
            kind,
            path,
            dataset.source.sheet.as_deref().unwrap_or(""),
            dataset.source.header_row.unwrap_or(1).max(1)
        );
    }
    let resolved_identifier = resolve_versioned_source_identifier(app_root, path);
    let absolute_path = app_root.join(&resolved_identifier);
    let content_signature = resolve_data_snapshot_import_entry(
        app_root,
        path,
        dataset.source.sheet.as_deref(),
        dataset.source.header_row.unwrap_or(1).max(1) as usize,
    )
    .map(|entry| format!("import:{}", entry.content_signature))
    .unwrap_or_else(|| {
        format!(
            "source:{}",
            source_file_content_signature(absolute_path.as_path(), resolved_identifier.as_str())
        )
    });
    format!(
        "{}|kind={}|path={}|content_sig={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        resolved_identifier,
        content_signature,
        dataset.source.sheet.as_deref().unwrap_or(""),
        dataset.source.header_row.unwrap_or(1).max(1)
    )
}

fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
}

pub(crate) fn dataset_metric_identity_key(dataset: &DatasetView) -> String {
    let mut metric_keys = dataset
        .runtime_metric_defs
        .keys()
        .map(|metric_id| metric_id.as_str())
        .collect::<Vec<_>>();
    metric_keys.sort_unstable();
    let source_path = dataset.source.path.trim().replace('\\', "/");
    format!("{source_path}|{}", metric_keys.join(","))
}

pub(crate) fn dataset_resource_lookup_aliases(dataset_id: &str) -> Vec<String> {
    let trimmed = dataset_id.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![trimmed.to_string()];
    if let Some(_capsule_path) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(trimmed)
    {
        if !aliases.iter().any(|id| id == "__world_metrics__") {
            aliases.push("__world_metrics__".to_string());
        }
    }
    if let Some((_, bare)) = trimmed.rsplit_once("::") {
        let bare = bare.trim();
        if !bare.is_empty() && !aliases.iter().any(|id| id == bare) {
            aliases.push(bare.to_string());
        }
    }
    aliases
}

pub(crate) fn equivalent_dataset_resource_ids(
    compiled: &CompiledApp,
    owner_dataset: &DatasetView,
) -> Vec<String> {
    let identity = dataset_metric_identity_key(owner_dataset);
    let mut ids = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            (dataset_metric_identity_key(dataset) == identity).then(|| resource.id.clone())
        })
        .collect::<Vec<_>>();
    for alias in dataset_resource_lookup_aliases(owner_dataset.id.as_str()) {
        if !ids.iter().any(|id| id == &alias) {
            ids.push(alias);
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

