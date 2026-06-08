use std::collections::BTreeMap;

use mei_lang_kernel::{
    DatasetView, DimensionBinding, FilterIntent, FilterIntentSource, FilterOperator, QueryState,
    QueryTimeRange,
};

pub(crate) fn normalize_query_search(search: Option<&str>) -> Option<String> {
    let value = search.unwrap_or("").trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn normalize_query_filters(
    filters: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut normalized = BTreeMap::new();
    for (key, value) in filters {
        let normalized_key = key.trim();
        let normalized_value = value.trim();
        if normalized_key.is_empty() || normalized_value.is_empty() {
            continue;
        }
        normalized.insert(normalized_key.to_string(), normalized_value.to_string());
    }
    normalized
}

pub(crate) fn normalize_query_group(group: &[String]) -> Vec<String> {
    let mut normalized = group
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub(crate) fn normalize_query_time_range(
    time_range: Option<&QueryTimeRange>,
) -> Option<QueryTimeRange> {
    let raw = time_range?;
    let dimension = raw
        .dimension
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let start = raw
        .start
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let end = raw
        .end
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let preset = raw
        .preset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if dimension.is_none() && start.is_none() && end.is_none() && preset.is_none() {
        return None;
    }
    Some(QueryTimeRange {
        dimension,
        start,
        end,
        preset,
    })
}

pub(crate) fn query_state_from_filters(
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> QueryState {
    QueryState {
        filters: normalize_query_filters(filters),
        search: normalize_query_search(search),
        group: Vec::new(),
        time_range: None,
    }
}

pub(crate) fn query_state_from_request(
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
    state: Option<&QueryState>,
) -> QueryState {
    let mut merged = query_state_from_filters(filters, search);
    if let Some(state) = state {
        for (dimension, value) in normalize_query_filters(&state.filters) {
            merged.filters.insert(dimension, value);
        }
        if state.search.is_some() {
            merged.search = normalize_query_search(state.search.as_deref());
        }
        if !state.group.is_empty() {
            merged.group = normalize_query_group(&state.group);
        }
        if state.time_range.is_some() {
            merged.time_range = normalize_query_time_range(state.time_range.as_ref());
        }
    }
    merged
}

pub(crate) fn filter_intents_from_query_state(
    state: &QueryState,
    source: FilterIntentSource,
) -> Vec<FilterIntent> {
    state
        .filters
        .iter()
        .map(|(dimension, value)| FilterIntent {
            dimension: dimension.clone(),
            operator: FilterOperator::Eq,
            value: value.clone(),
            source,
        })
        .collect()
}

pub(crate) fn normalize_filter_intents(intents: &[FilterIntent]) -> Vec<FilterIntent> {
    intents
        .iter()
        .filter_map(|intent| {
            let dimension = intent.dimension.trim();
            let value = intent.value.trim();
            if dimension.is_empty() || value.is_empty() {
                return None;
            }
            Some(FilterIntent {
                dimension: dimension.to_string(),
                operator: intent.operator,
                value: value.to_string(),
                source: intent.source,
            })
        })
        .collect()
}

pub(crate) fn filter_intents_from_request(
    state: &QueryState,
    intents: &[FilterIntent],
) -> Vec<FilterIntent> {
    let normalized = normalize_filter_intents(intents);
    if !normalized.is_empty() {
        return normalized;
    }
    filter_intents_from_query_state(state, FilterIntentSource::QueryState)
}

pub(crate) fn dimension_bindings_from_query_state(state: &QueryState) -> Vec<DimensionBinding> {
    state
        .filters
        .keys()
        .map(|dimension| DimensionBinding {
            dimension: dimension.clone(),
            field: dimension.clone(),
        })
        .collect()
}

pub(crate) fn dimension_bindings_from_query_state_for_dataset(
    state: &QueryState,
    dataset: &DatasetView,
) -> Vec<DimensionBinding> {
    use crate::metric_hydrate::dataset_dimension_bindings;
    let catalog = dataset_dimension_bindings(dataset);
    state
        .filters
        .keys()
        .filter_map(|dimension| {
            let normalized = dimension.trim();
            if normalized.is_empty() {
                return None;
            }
            catalog
                .iter()
                .find(|binding| binding.dimension == normalized)
                .map(|binding| DimensionBinding {
                    dimension: normalized.to_string(),
                    field: binding.field.clone(),
                })
        })
        .collect()
}
