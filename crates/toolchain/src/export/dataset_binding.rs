use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    resolve_runtime_metric_def_key, CompiledApp, DatasetView, DimensionBinding, FilterIntent,
    FilterIntentSource, FilterOperator, QueryState,
};

pub(crate) fn collect_dataset_views(compiled: &CompiledApp) -> BTreeMap<String, DatasetView> {
    compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .as_ref()
                .map(|dataset| (dataset.id.clone(), dataset.clone()))
        })
        .collect()
}

pub(crate) fn normalize_filters(filters: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    filters
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}

pub(crate) fn normalize_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn query_state(filters: &BTreeMap<String, String>, search: Option<&str>) -> QueryState {
    QueryState {
        filters: normalize_filters(filters),
        search: normalize_search(search),
        group: Vec::new(),
        time_range: None,
    }
}

pub(crate) fn filter_intents(state: &QueryState) -> Vec<FilterIntent> {
    state
        .filters
        .iter()
        .map(|(dimension, value)| FilterIntent {
            dimension: dimension.clone(),
            operator: FilterOperator::Eq,
            value: value.clone(),
            source: FilterIntentSource::QueryState,
        })
        .collect()
}

fn dataset_field_names(dataset: &DatasetView) -> BTreeSet<String> {
    let mut fields = dataset.columns.iter().cloned().collect::<BTreeSet<_>>();
    for column in &dataset.schema {
        fields.insert(column.name.clone());
        if let Some(source) = column.source.as_ref() {
            fields.insert(source.clone());
        }
    }
    fields
}

pub(crate) fn dimension_bindings(
    dataset: &DatasetView,
    state: &QueryState,
) -> Result<Vec<DimensionBinding>> {
    let fields = dataset_field_names(dataset);
    let mut bindings = Vec::new();
    for dimension in state.filters.keys() {
        if fields.contains(dimension) {
            bindings.push(DimensionBinding {
                dimension: dimension.clone(),
                field: dimension.clone(),
            });
            continue;
        }
        let fallback = fields
            .iter()
            .find(|field| field.eq_ignore_ascii_case(dimension))
            .cloned();
        if let Some(field) = fallback {
            bindings.push(DimensionBinding {
                dimension: dimension.clone(),
                field,
            });
        } else {
            anyhow::bail!(
                "filter dimension `{dimension}` is not available on dataset `{}`",
                dataset.id
            );
        }
    }
    Ok(bindings)
}

pub(crate) fn resolve_metric_ids(
    dataset: &DatasetView,
    requested_metric_ids: &[String],
) -> Result<Vec<String>> {
    if requested_metric_ids.is_empty() {
        return Ok(dataset.runtime_metric_defs.keys().cloned().collect());
    }
    requested_metric_ids
        .iter()
        .map(|metric_id| {
            resolve_runtime_metric_def_key(&dataset.id, metric_id, &dataset.runtime_metric_defs)
                .with_context(|| {
                    format!(
                        "failed to resolve runtime metric `{metric_id}` for dataset `{}`",
                        dataset.id
                    )
                })
        })
        .collect()
}
