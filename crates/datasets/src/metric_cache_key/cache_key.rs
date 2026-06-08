use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_versioned_source_identifier, DatasetView, FilterIntent,
    QueryState, RuntimeMetricEvalScope,
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
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
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
