pub(crate) fn stable_slot_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn effective_compile_revision_for_slot(
    compile_revision: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
) -> String {
    if !graph_slot_revision_enabled() {
        return compile_revision.to_string();
    }
    let body = format!(
        "mdb={metric_def_bundle_revision}\nds={data_source_revision}\nscope={scope_key}\nengine=json_walk"
    );
    format!("sr:{}", stable_slot_hash(&body))
}

pub(crate) fn metric_scope_cache_key(resolved_metric_ids: &[String]) -> String {
    if resolved_metric_ids.is_empty() {
        return "*".to_string();
    }
    let mut ids = resolved_metric_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    serialize_cache_value(&ids)
}

pub(crate) fn serialize_cache_value<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn filter_intents_fingerprint(scope: &RuntimeMetricEvalScope) -> String {
    serialize_cache_value(&scope.filter_intents)
}

fn dimension_bindings_fingerprint(scope: &RuntimeMetricEvalScope) -> String {
    serialize_cache_value(&scope.dimension_bindings)
}

pub(crate) fn metric_request_revision_fingerprint(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    let mut dataset_ids = collect_dataset_ids_from_metric_defs(metric_defs);
    let base_dataset_id = base_dataset_id.trim();
    if !base_dataset_id.is_empty() {
        dataset_ids.insert(base_dataset_id.to_string());
    }
    let cache_key = revision_fingerprint_cache_key(app_root, datasets, &dataset_ids);
    if let Ok(cache) = revision_fingerprint_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.cached_at.elapsed() <= REVISION_FINGERPRINT_CACHE_TTL {
                return entry.value.clone();
            }
        }
    }
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    let value = format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
    );
    if let Ok(mut cache) = revision_fingerprint_cache().lock() {
        cache.insert(
            cache_key,
            RevisionFingerprintCacheEntry {
                value: value.clone(),
                cached_at: Instant::now(),
            },
        );
        if cache.len() > REVISION_FINGERPRINT_CACHE_MAX {
            let overflow = cache.len().saturating_sub(REVISION_FINGERPRINT_CACHE_MAX);
            if overflow > 0 {
                let keys = cache.keys().take(overflow).cloned().collect::<Vec<_>>();
                for key in keys {
                    cache.remove(&key);
                }
            }
        }
    }
    value
}

pub(crate) fn metric_request_revision_fingerprint_for_compiled(
    app_root: &Path,
    compiled: &CompiledApp,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    let mut dataset_ids = collect_dataset_ids_from_metric_defs(metric_defs);
    let base_dataset_id = base_dataset_id.trim();
    if !base_dataset_id.is_empty() {
        dataset_ids.insert(base_dataset_id.to_string());
    }
    let cache_key = revision_fingerprint_cache_key_for_compiled(app_root, compiled, &dataset_ids);
    if let Ok(cache) = revision_fingerprint_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.cached_at.elapsed() <= REVISION_FINGERPRINT_CACHE_TTL {
                return entry.value.clone();
            }
        }
    }
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_compiled_dataset_view(compiled, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    let value = format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
    );
    if let Ok(mut cache) = revision_fingerprint_cache().lock() {
        cache.insert(
            cache_key,
            RevisionFingerprintCacheEntry {
                value: value.clone(),
                cached_at: Instant::now(),
            },
        );
        if cache.len() > REVISION_FINGERPRINT_CACHE_MAX {
            let overflow = cache.len().saturating_sub(REVISION_FINGERPRINT_CACHE_MAX);
            if overflow > 0 {
                let keys = cache.keys().take(overflow).cloned().collect::<Vec<_>>();
                for key in keys {
                    cache.remove(&key);
                }
            }
        }
    }
    value
}

fn revision_fingerprint_cache_key(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    dataset_ids: &BTreeSet<String>,
) -> String {
    let mut items = dataset_ids
        .iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(dataset_source_cache_fingerprint)
        .collect::<Vec<_>>();
    items.sort();
    format!(
        "{}|materialize={}|deps={}",
        app_root.display(),
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&items)
    )
}

fn revision_fingerprint_cache_key_for_compiled(
    app_root: &Path,
    compiled: &CompiledApp,
    dataset_ids: &BTreeSet<String>,
) -> String {
    let mut items = dataset_ids
        .iter()
        .filter_map(|dataset_id| lookup_compiled_dataset_view(compiled, dataset_id.as_str()))
        .map(dataset_source_cache_fingerprint)
        .collect::<Vec<_>>();
    items.sort();
    format!(
        "{}|materialize={}|deps={}",
        app_root.display(),
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&items)
    )
}

fn dataset_source_cache_fingerprint(dataset: &DatasetView) -> String {
    let kind = dataset.source.kind.trim();
    let path = dataset.source.path.trim();
    format!(
        "{}|kind={}|path={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        path,
        dataset.source.sheet.as_deref().unwrap_or(""),
        dataset.source.header_row.unwrap_or(1).max(1)
    )
}

