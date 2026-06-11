use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use std::time::UNIX_EPOCH;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_versioned_source_identifier, CompiledApp, DatasetView,
    FilterIntent, QueryState, RuntimeMetricEvalScope,
};
use serde::Serialize;
use serde_json::Value;

use crate::metric_hydrate::{
    collect_dataset_ids_from_metric_defs, resolve_dataset_query_bindings_from_state,
};

use super::query_normalize::{
    dimension_bindings_from_query_state, dimension_bindings_from_query_state_for_dataset,
    filter_intents_from_request, normalize_query_filters, query_state_from_request,
};

const REVISION_FINGERPRINT_CACHE_TTL: Duration = Duration::from_millis(4000);
const REVISION_FINGERPRINT_CACHE_MAX: usize = 512;

#[derive(Clone)]
struct RevisionFingerprintCacheEntry {
    value: String,
    cached_at: Instant,
}

fn revision_fingerprint_cache() -> &'static Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
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

pub(crate) fn runtime_metric_eval_scope(
    binding_dataset: Option<&DatasetView>,
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
    let dimension_bindings = binding_dataset
        .map(|dataset| {
            validate_runtime_scope_bindings(&query_state, dataset)?;
            Ok::<_, anyhow::Error>(dimension_bindings_from_query_state_for_dataset(
                &query_state,
                dataset,
            ))
        })
        .transpose()?
        .unwrap_or_else(|| dimension_bindings_from_query_state(&query_state));
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

fn validate_runtime_scope_bindings(state: &QueryState, dataset: &DatasetView) -> Result<()> {
    let resolution = resolve_dataset_query_bindings_from_state(state, dataset);
    if !resolution.unresolved_filter_dimensions.is_empty() {
        return Err(anyhow!(
            "runtime metric query requires resolvable filter bindings for dataset `{}`: {}",
            dataset.id,
            resolution.unresolved_filter_dimensions.join(", ")
        ));
    }
    if let Some(dimension) = resolution.unresolved_time_range_dimension {
        return Err(anyhow!(
            "runtime metric query requires resolvable time_range.dimension binding for dataset `{}`: {}",
            dataset.id,
            dimension
        ));
    }
    Ok(())
}

pub(crate) fn eval_node_cache_key(
    expr_fingerprint: &str,
    scope: &RuntimeMetricEvalScope,
) -> String {
    format!(
        "expr={}|dataset={}|scene={}|target={}|search={}|filters={}|group={}|time_range={}|deps={}",
        expr_fingerprint.trim(),
        scope.base_dataset_id.trim(),
        scope.scene_id.trim(),
        scope.target.trim(),
        scope.search.trim(),
        scope.filters_fingerprint.trim(),
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
    let modified_ms = std::fs::metadata(&absolute_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!(
        "{}|kind={}|path={}|mtime={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        resolved_identifier,
        modified_ms,
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

fn lookup_compiled_dataset_view<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    compiled.resources.iter().find_map(|resource| {
        let dataset = resource.dataset.as_ref()?;
        (resource.id == normalized
            || dataset.id == normalized
            || resource.id.ends_with(&format!("::{normalized}"))
            || resource.id.ends_with(&format!("/{normalized}")))
        .then_some(dataset)
    })
}
