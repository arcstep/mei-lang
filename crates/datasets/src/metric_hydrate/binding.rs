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

fn dataset_column_names(datasets: &[&DatasetView]) -> BTreeSet<String> {
    let mut columns = BTreeSet::new();
    for dataset in datasets {
        for name in &dataset.columns {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                columns.insert(trimmed.to_string());
            }
        }
        for column in dataset.schema.iter().chain(dataset.stage_schema.iter()) {
            let trimmed = column.name.trim();
            if !trimmed.is_empty() {
                columns.insert(trimmed.to_string());
            }
        }
    }
    columns
}

/// Cockpit object_focus / drilldown often sends logical keys (`resultId`, `value`)
/// that never land in compiled `filter_dimensions`. Map them onto real columns so
/// SQL page pushdown does not fail closed as `sql_page_with_filters`.
fn resolve_fallback_filter_field(
    key: &str,
    value: &str,
    columns: &BTreeSet<String>,
) -> Option<String> {
    let pick = |candidates: &[&str]| -> Option<String> {
        candidates
            .iter()
            .find(|name| columns.contains(**name))
            .map(|name| (*name).to_string())
    };
    match key {
        "resultId" | "result_id" | "caseResultId" => {
            if value.contains('-') {
                pick(&["处理结果ID-问题跟踪ID", "处理结果ID"])
            } else {
                pick(&["处理结果ID", "处理结果ID-问题跟踪ID"])
            }
        }
        "warningId" | "warning_id" => pick(&["预警ID"]),
        "modelId" | "model_id" => pick(&["模型ID"]),
        "matterId" | "matter_id" => pick(&["序号"]),
        "value" => {
            if columns.contains("处理结果ID-问题跟踪ID") || columns.contains("处理结果ID") {
                if value.contains('-') {
                    pick(&["处理结果ID-问题跟踪ID", "处理结果ID"])
                } else {
                    pick(&["处理结果ID", "处理结果ID-问题跟踪ID"])
                }
            } else if columns.contains("模型ID") {
                pick(&["模型ID"])
            } else if columns.contains("预警ID") {
                pick(&["预警ID"])
            } else {
                pick(&["序号"])
            }
        }
        _ => None,
    }
}

/// Map logical filter keys (e.g. `warningType` / `resultId`) onto dataset column fields
/// using schema / `filter_dimensions`, then identity-alias fallbacks.
/// Unknown non-column keys are dropped (keeping them breaks SQL page with filters).
pub(crate) fn remap_filters_to_dataset_fields(
    filters: &BTreeMap<String, String>,
    datasets: &[&DatasetView],
) -> BTreeMap<String, String> {
    let columns = dataset_column_names(datasets);
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
        let field = mapped_field
            .or_else(|| {
                // Prefer primary/owner order from caller. For generic `value` (object
                // identity alias), only the first dataset with usable columns may map —
                // scanning BTreeMap sql_datasets would steal e.g. issue_result columns
                // onto a model-detail query and break SQL page.
                if key == "value" {
                    // primary (+ owner). Do not scan referenced sql_datasets — BTreeMap
                    // order can map value onto the wrong table (issue_result vs models).
                    for dataset in datasets.iter().take(2) {
                        let cols = dataset_column_names(&[*dataset]);
                        if cols.is_empty() {
                            continue;
                        }
                        return resolve_fallback_filter_field(key, value, &cols);
                    }
                    return None;
                }
                for dataset in datasets {
                    let cols = dataset_column_names(&[dataset]);
                    if let Some(field) = resolve_fallback_filter_field(key, value, &cols) {
                        return Some(field);
                    }
                }
                None
            })
            .or_else(|| {
                if columns.contains(key) {
                    Some(key.to_string())
                } else {
                    None
                }
            });
        match field {
            Some(field) => {
                out.insert(field, value.to_string());
            }
            None => {
                tracing::warn!(
                    filter_key = %key,
                    "dropping unresolved metric dataframe filter key (not a dataset column)"
                );
            }
        }
    }
    out
}
