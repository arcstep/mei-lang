use std::collections::{BTreeMap, HashSet};

use mei_lang_datasets::{normalize_query_filters, normalize_query_search};
use mei_lang_kernel::{FilterIntent, FilterIntentSource, FilterOperator, QueryState};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub(crate) struct AccessBrowserState {
    pub active_query_state_ids: Vec<String>,
    pub merged_query_state: Option<QueryState>,
    pub filter_intents: Vec<FilterIntent>,
}

fn parse_source(value: Option<&str>) -> FilterIntentSource {
    match value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("query_state") => FilterIntentSource::QueryState,
        Some("filter_bar") => FilterIntentSource::FilterBar,
        Some("metric_click") => FilterIntentSource::MetricClick,
        Some("chart_selection") => FilterIntentSource::ChartSelection,
        Some("table_selection") => FilterIntentSource::TableSelection,
        Some("drilldown") => FilterIntentSource::Drilldown,
        _ => FilterIntentSource::Unknown,
    }
}

fn parse_filters(value: Option<&Value>) -> BTreeMap<String, String> {
    let Some(raw) = value.and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    let mut filters = BTreeMap::new();
    for (key, item) in raw {
        let normalized_key = key.trim();
        let normalized_value = item.as_str().map(str::trim).unwrap_or_default();
        if normalized_key.is_empty() || normalized_value.is_empty() {
            continue;
        }
        filters.insert(normalized_key.to_string(), normalized_value.to_string());
    }
    normalize_query_filters(&filters)
}

fn parse_filter_intents(value: Option<&Value>) -> Vec<FilterIntent> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut intents = Vec::new();
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        let dimension = map
            .get("dimension")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let filter_value = map
            .get("value")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if dimension.is_empty() || filter_value.is_empty() {
            continue;
        }
        intents.push(FilterIntent {
            dimension: dimension.to_string(),
            operator: FilterOperator::Eq,
            value: filter_value.to_string(),
            source: parse_source(map.get("source").and_then(Value::as_str)),
        });
    }
    intents
}

fn parse_query_state_entry(value: &Value) -> Option<(String, QueryState, Vec<FilterIntent>)> {
    let map = value.as_object()?;
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())?
        .to_string();
    let filters = parse_filters(map.get("filters"));
    let search = normalize_query_search(map.get("search").and_then(Value::as_str));
    let query_state = QueryState {
        filters,
        search,
        group: Vec::new(),
        time_range: None,
    };
    let filter_intents = parse_filter_intents(map.get("filter_intents"));
    Some((id, query_state, filter_intents))
}

fn dedupe_filter_intents(intents: Vec<FilterIntent>) -> Vec<FilterIntent> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in intents {
        let key = format!(
            "{}|{}|{:?}|{:?}",
            item.dimension, item.value, item.operator, item.source
        );
        if seen.insert(key) {
            out.push(item);
        }
    }
    out
}

fn query_state_has_content(state: &QueryState) -> bool {
    !state.filters.is_empty()
        || state
            .search
            .as_deref()
            .map(str::trim)
            .is_some_and(|item| !item.is_empty())
        || !state.group.is_empty()
        || state.time_range.is_some()
}

pub(crate) fn access_browser_state(browser_context: Option<&Value>) -> AccessBrowserState {
    let Some(ctx) = browser_context.and_then(Value::as_object) else {
        return AccessBrowserState::default();
    };
    let active_query_state_ids = ctx
        .get("active_query_state_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let parsed_states = ctx
        .get("query_states")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_query_state_entry)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected = if active_query_state_ids.is_empty() {
        parsed_states
    } else {
        active_query_state_ids
            .iter()
            .filter_map(|id| {
                parsed_states
                    .iter()
                    .find(|(entry_id, _, _)| entry_id == id)
                    .cloned()
            })
            .collect::<Vec<_>>()
    };
    let mut merged_query_state = QueryState::default();
    let mut merged_filter_intents = Vec::new();
    for (_, state, intents) in selected {
        for (dimension, value) in state.filters {
            merged_query_state.filters.insert(dimension, value);
        }
        if state.search.is_some() {
            merged_query_state.search = state.search;
        }
        if !state.group.is_empty() {
            merged_query_state.group = state.group;
        }
        if state.time_range.is_some() {
            merged_query_state.time_range = state.time_range;
        }
        merged_filter_intents.extend(intents);
    }
    let merged_filter_intents = if merged_filter_intents.is_empty() {
        merged_query_state
            .filters
            .iter()
            .map(|(dimension, value)| FilterIntent {
                dimension: dimension.clone(),
                operator: FilterOperator::Eq,
                value: value.clone(),
                source: FilterIntentSource::QueryState,
            })
            .collect::<Vec<_>>()
    } else {
        dedupe_filter_intents(merged_filter_intents)
    };
    AccessBrowserState {
        active_query_state_ids,
        merged_query_state: query_state_has_content(&merged_query_state)
            .then_some(merged_query_state),
        filter_intents: merged_filter_intents,
    }
}

pub(crate) fn merge_browser_query_state_with_args(
    browser_query_state: Option<&QueryState>,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> QueryState {
    let mut merged = browser_query_state.cloned().unwrap_or_default();
    for (dimension, value) in normalize_query_filters(filters) {
        merged.filters.insert(dimension, value);
    }
    if let Some(normalized_search) = normalize_query_search(search) {
        merged.search = Some(normalized_search);
    }
    merged
}

pub(crate) fn effective_filter_intents(
    browser_filter_intents: &[FilterIntent],
    query_state: &QueryState,
) -> Vec<FilterIntent> {
    if !browser_filter_intents.is_empty() {
        return dedupe_filter_intents(browser_filter_intents.to_vec());
    }
    query_state
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{
        access_browser_state, effective_filter_intents, merge_browser_query_state_with_args,
    };

    #[test]
    fn browser_query_state_merges_active_entries() {
        let browser = json!({
            "active_query_state_ids": ["qs_region", "qs_status"],
            "query_states": [
                {
                    "id": "qs_region",
                    "filters": { "region": "华东" },
                    "search": "火灾"
                },
                {
                    "id": "qs_status",
                    "filters": { "status": "处理中" },
                    "filter_intents": [
                        {
                            "dimension": "status",
                            "value": "处理中",
                            "source": "filter_bar"
                        }
                    ]
                }
            ]
        });
        let state = access_browser_state(Some(&browser));
        let merged = state.merged_query_state.expect("merged query state");
        assert_eq!(
            merged.filters.get("region").map(String::as_str),
            Some("华东")
        );
        assert_eq!(
            merged.filters.get("status").map(String::as_str),
            Some("处理中")
        );
        assert_eq!(merged.search.as_deref(), Some("火灾"));
        assert_eq!(state.filter_intents.len(), 1);
    }

    #[test]
    fn explicit_args_override_browser_query_state() {
        let browser = json!({
            "active_query_state_ids": ["qs_region"],
            "query_states": [
                {
                    "id": "qs_region",
                    "filters": { "region": "华东" },
                    "search": "旧值"
                }
            ]
        });
        let browser_state = access_browser_state(Some(&browser));
        let mut filters = BTreeMap::new();
        filters.insert("region".to_string(), "华北".to_string());
        let merged = merge_browser_query_state_with_args(
            browser_state.merged_query_state.as_ref(),
            &filters,
            Some("新值"),
        );
        assert_eq!(
            merged.filters.get("region").map(String::as_str),
            Some("华北")
        );
        assert_eq!(merged.search.as_deref(), Some("新值"));
    }

    #[test]
    fn derives_filter_intents_when_browser_context_has_none() {
        let browser = json!({
            "active_query_state_ids": ["qs_region"],
            "query_states": [
                {
                    "id": "qs_region",
                    "filters": { "region": "华东" }
                }
            ]
        });
        let browser_state = access_browser_state(Some(&browser));
        let query_state = browser_state.merged_query_state.expect("query state");
        let intents = effective_filter_intents(&browser_state.filter_intents, &query_state);
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].dimension, "region");
        assert_eq!(intents[0].value, "华东");
    }
}
