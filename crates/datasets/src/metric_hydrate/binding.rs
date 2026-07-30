use std::collections::{BTreeMap, BTreeSet};

use mei_lang_kernel::{DatasetView, DimensionBinding, QueryState};

use crate::types::{parse_source_meta, DatasetQueryOptions};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DatasetQueryBindingResolution {
    pub mapped_filters: BTreeMap<String, String>,
    pub unresolved_filter_dimensions: Vec<String>,
    pub unresolved_time_range_dimension: Option<String>,
}

pub(crate) fn unique_dataset_views<'a>(
    primary: &'a DatasetView,
    others: impl IntoIterator<Item = &'a DatasetView>,
) -> Vec<&'a DatasetView> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for dataset in std::iter::once(primary).chain(others) {
        if seen.insert(dataset.id.clone()) {
            out.push(dataset);
        }
    }
    out
}

pub(crate) fn compatible_hydrate_binding_resolution(
    query: &DatasetQueryOptions,
    dataset: &DatasetView,
) -> DatasetQueryBindingResolution {
    resolve_dataset_query_bindings_from_state(
        &QueryState {
            filters: query.filters.clone(),
            search: query.search.clone(),
            group: query.group.clone(),
            time_range: query.time_range.clone(),
        },
        dataset,
    )
}

pub(crate) fn resolve_dataset_query_bindings_from_state(
    state: &QueryState,
    dataset: &DatasetView,
) -> DatasetQueryBindingResolution {
    let bindings = dataset_dimension_bindings(dataset);
    let mut mapped_filters = BTreeMap::new();
    let mut unresolved_filter_dimensions = Vec::new();
    for (key, value) in &state.filters {
        let normalized = key.trim();
        if normalized.is_empty() {
            continue;
        }
        if let Some(binding) = resolve_filter_binding(bindings.as_slice(), normalized) {
            mapped_filters.insert(binding.field.clone(), value.clone());
        } else {
            unresolved_filter_dimensions.push(normalized.to_string());
        }
    }
    unresolved_filter_dimensions.sort();
    unresolved_filter_dimensions.dedup();
    let unresolved_time_range_dimension = state
        .time_range
        .as_ref()
        .and_then(|time_range| time_range.dimension.as_deref())
        .map(str::trim)
        .filter(|dimension| !dimension.is_empty())
        .filter(|dimension| resolve_filter_binding(bindings.as_slice(), dimension).is_none())
        .map(str::to_string);
    DatasetQueryBindingResolution {
        mapped_filters,
        unresolved_filter_dimensions,
        unresolved_time_range_dimension,
    }
}

pub(crate) fn dataset_dimension_bindings(dataset: &DatasetView) -> Vec<DimensionBinding> {
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::new();
    let mut push_binding = |dimension: &str, field: &str| {
        let normalized_dimension = dimension.trim();
        let normalized_field = field.trim();
        if normalized_dimension.is_empty() || normalized_field.is_empty() {
            return;
        }
        if !seen.insert((
            normalized_dimension.to_string(),
            normalized_field.to_string(),
        )) {
            return;
        }
        bindings.push(DimensionBinding {
            dimension: normalized_dimension.to_string(),
            field: normalized_field.to_string(),
        });
    };
    for name in &dataset.columns {
        push_binding(name, name);
    }
    for column in &dataset.schema {
        push_binding(&column.name, &column.name);
    }
    for column in &dataset.stage_schema {
        push_binding(&column.name, &column.name);
    }
    let meta = parse_source_meta(dataset.source.content.as_deref());
    for name in meta.normalize.values() {
        push_binding(name, name);
    }
    for (dimension, field) in &meta.filter_dimensions {
        push_binding(dimension, field);
    }
    bindings
}

pub(crate) fn dimension_bindings_from_query_state_for_datasets(
    state: &QueryState,
    datasets: &[&DatasetView],
) -> Vec<DimensionBinding> {
    let mut bindings = Vec::new();
    let mut seen = BTreeSet::new();
    for (dimension, _) in &state.filters {
        let normalized = dimension.trim();
        if normalized.is_empty() || !seen.insert(normalized.to_string()) {
            continue;
        }
        for dataset in datasets {
            let catalog = dataset_dimension_bindings(dataset);
            let Some(binding) = resolve_filter_binding(catalog.as_slice(), normalized) else {
                continue;
            };
            bindings.push(DimensionBinding {
                dimension: normalized.to_string(),
                field: binding.field.clone(),
            });
            break;
        }
    }
    bindings
}

pub(crate) fn unresolved_filter_dimensions_for_datasets(
    state: &QueryState,
    datasets: &[&DatasetView],
) -> Vec<String> {
    let mut unresolved = Vec::new();
    for (dimension, _) in &state.filters {
        let normalized = dimension.trim();
        if normalized.is_empty() {
            continue;
        }
        let resolves = datasets.iter().any(|dataset| {
            let catalog = dataset_dimension_bindings(dataset);
            resolve_filter_binding(catalog.as_slice(), normalized).is_some()
        });
        if !resolves {
            unresolved.push(normalized.to_string());
        }
    }
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

fn resolve_filter_binding<'a>(
    bindings: &'a [DimensionBinding],
    dimension: &str,
) -> Option<&'a DimensionBinding> {
    let normalized = dimension.trim();
    if normalized.is_empty() {
        return None;
    }
    bindings
        .iter()
        .find(|binding| binding.dimension == normalized)
}

/// Map logical filter keys (e.g. `warningType`) onto dataset column fields using
/// schema / `filter_dimensions`. Unresolvable keys are kept as-is.
pub(crate) fn remap_filters_to_dataset_fields(
    filters: &BTreeMap<String, String>,
    datasets: &[&DatasetView],
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in filters {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        let probe = QueryState {
            filters: BTreeMap::from([(key.to_string(), value.to_string())]),
            search: None,
            group: Vec::new(),
            time_range: None,
        };
        let mut mapped_field = None;
        for dataset in datasets {
            let resolution = resolve_dataset_query_bindings_from_state(&probe, dataset);
            if !resolution.unresolved_filter_dimensions.is_empty() {
                continue;
            }
            if let Some(field) = resolution.mapped_filters.keys().next() {
                mapped_field = Some(field.clone());
                break;
            }
        }
        out.insert(
            mapped_field.unwrap_or_else(|| key.to_string()),
            value.to_string(),
        );
    }
    out
}
