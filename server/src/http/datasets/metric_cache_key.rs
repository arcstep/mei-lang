use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_runtime_metric_def_key, resolve_versioned_source_identifier,
    DatasetView, DimensionBinding, FilterIntent, FilterIntentSource, FilterOperator, QueryState,
    QueryTimeRange, RuntimeMetricEvalScope, runtime_analysis_closure_metric_ids,
};
use serde::Serialize;
use serde_json::Value;

use super::metric_hydrate::{
    collect_dataset_ids_from_metric_defs,
    resolve_dataset_query_bindings_from_state,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeMetricWorkset {
    pub resolved_metric_ids: Vec<String>,
    pub closure_metric_ids: Vec<String>,
    pub eval_metric_ids: Option<Vec<String>>,
    pub defs_for_hydrate: BTreeMap<String, Value>,
}

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

pub(crate) fn normalize_query_time_range(time_range: Option<&QueryTimeRange>) -> Option<QueryTimeRange> {
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

pub(crate) fn resolve_runtime_metric_ids(
    resource_id: &str,
    requested_metric_ids: &[String],
    defs: &BTreeMap<String, Value>,
) -> Vec<String> {
    requested_metric_ids
        .iter()
        .filter_map(|metric_id| resolve_runtime_metric_def_key(resource_id, metric_id, defs))
        .collect()
}

pub(crate) fn select_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    resolved_metric_ids: &[String],
) -> BTreeMap<String, Value> {
    if resolved_metric_ids.is_empty() {
        return metric_defs.clone();
    }
    resolved_metric_ids
        .iter()
        .filter_map(|metric_id| {
            metric_defs
                .get(metric_id)
                .cloned()
                .map(|value| (metric_id.clone(), value))
        })
        .collect()
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

pub(crate) fn runtime_metric_workset(
    resource_id: &str,
    requested_metric_ids: &[String],
    dataset: &DatasetView,
) -> RuntimeMetricWorkset {
    let resolved_metric_ids =
        resolve_runtime_metric_ids(resource_id, requested_metric_ids, &dataset.runtime_metric_defs);
    if requested_metric_ids.is_empty() {
        return RuntimeMetricWorkset {
            resolved_metric_ids,
            closure_metric_ids: Vec::new(),
            eval_metric_ids: None,
            defs_for_hydrate: dataset.runtime_metric_defs.clone(),
        };
    }
    let closure_metric_ids =
        runtime_analysis_closure_metric_ids(&dataset.runtime_analysis_graph, &resolved_metric_ids);
    let eval_metric_ids = if closure_metric_ids.is_empty() {
        resolved_metric_ids.clone()
    } else {
        closure_metric_ids.clone()
    };
    RuntimeMetricWorkset {
        resolved_metric_ids,
        closure_metric_ids,
        defs_for_hydrate: select_metric_defs(&dataset.runtime_metric_defs, &eval_metric_ids),
        eval_metric_ids: Some(eval_metric_ids),
    }
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
    use super::metric_hydrate::dataset_dimension_bindings;
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

pub(crate) fn eval_node_cache_key(expr_fingerprint: &str, scope: &RuntimeMetricEvalScope) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::SourceDecl;

    #[test]
    fn metric_scope_cache_key_sorts_and_dedups() {
        let value = metric_scope_cache_key(&[
            "b".to_string(),
            "a".to_string(),
            "b".to_string(),
        ]);
        assert_eq!(value, "[\"a\",\"b\"]");
    }

    #[test]
    fn metric_request_revision_fingerprint_includes_base_dataset() {
        let mut datasets = BTreeMap::new();
        datasets.insert(
            "sample".to_string(),
            DatasetView {
                id: "sample".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: Vec::new(),
                source: SourceDecl {
                    kind: "derived".to_string(),
                    path: "legacy.metric_pack:sample".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            },
        );
        let fingerprint = metric_request_revision_fingerprint(
            Path::new("/tmp"),
            &datasets,
            "sample",
            &BTreeMap::new(),
        );
        assert!(fingerprint.contains("sample"));
    }

    #[test]
    fn eval_node_cache_key_contains_scope_dimensions() {
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: "abc".to_string(),
            query_state: QueryState {
                filters: BTreeMap::from([("status".to_string(), "待办".to_string())]),
                search: Some("abc".to_string()),
                group: vec!["park".to_string()],
                time_range: Some(QueryTimeRange {
                    dimension: Some("created_at".to_string()),
                    start: Some("2024-01-01".to_string()),
                    end: Some("2024-12-31".to_string()),
                    preset: Some("year".to_string()),
                }),
            },
            filter_intents: vec![FilterIntent {
                dimension: "status".to_string(),
                operator: FilterOperator::Eq,
                value: "待办".to_string(),
                source: FilterIntentSource::QueryState,
            }],
            dimension_bindings: vec![DimensionBinding {
                dimension: "status".to_string(),
                field: "status".to_string(),
            }],
            filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
            dependency_revision_key: "deps=v1".to_string(),
        };
        let key = eval_node_cache_key("expr:count(rowset)", &scope);
        assert!(key.contains("expr=expr:count(rowset)"));
        assert!(key.contains("dataset=warning_list"));
        assert!(key.contains("scene=home"));
        assert!(key.contains("target=scenes/home.mei"));
        assert!(key.contains("search=abc"));
        assert!(key.contains("filters={\"status\":\"待办\"}"));
        assert!(key.contains("group=[\"park\"]"));
        assert!(
            key.contains(
                "time_range={\"dimension\":\"created_at\",\"start\":\"2024-01-01\",\"end\":\"2024-12-31\",\"preset\":\"year\"}"
            )
        );
        assert!(key.contains("deps=deps=v1"));
    }

    #[test]
    fn normalize_query_filters_drops_empty_and_trims() {
        let raw = BTreeMap::from([
            (" status ".to_string(), " 待办 ".to_string()),
            ("empty".to_string(), "".to_string()),
            ("  ".to_string(), "x".to_string()),
        ]);
        let normalized = normalize_query_filters(&raw);
        assert_eq!(normalized.get("status"), Some(&"待办".to_string()));
        assert!(!normalized.contains_key("empty"));
        assert_eq!(normalized.len(), 1);
    }

    #[test]
    fn normalize_query_search_trims_blank() {
        assert_eq!(normalize_query_search(Some("  abc ")), Some("abc".to_string()));
        assert_eq!(normalize_query_search(Some("   ")), None);
        assert_eq!(normalize_query_search(None), None);
    }

    #[test]
    fn runtime_metric_eval_scope_materializes_query_state_filter_intents_and_bindings() {
        let filters = BTreeMap::from([(" status ".to_string(), " 待办 ".to_string())]);
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["status".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:warning_list".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: Some(r#"{"normalize":{"原状态":"status"}}"#.to_string()),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let scope = runtime_metric_eval_scope(
            Some(&dataset),
            "warning_list",
            "home",
            Some("scenes/home.mei"),
            Some("abc"),
            &filters,
            None,
            &[],
            "deps=v1",
        )
        .expect("runtime metric eval scope");
        assert_eq!(
            scope.query_state.filters,
            BTreeMap::from([("status".to_string(), "待办".to_string())])
        );
        assert_eq!(scope.query_state.search.as_deref(), Some("abc"));
        assert_eq!(scope.query_state.group, Vec::<String>::new());
        assert_eq!(scope.query_state.time_range, None);
        assert_eq!(scope.filter_intents.len(), 1);
        assert_eq!(scope.filter_intents[0].dimension, "status");
        assert_eq!(scope.filter_intents[0].value, "待办");
        assert_eq!(scope.dimension_bindings.len(), 1);
        assert_eq!(scope.dimension_bindings[0].dimension, "status");
        assert_eq!(scope.dimension_bindings[0].field, "status");
    }

    #[test]
    fn runtime_metric_eval_scope_prefers_host_supplied_filter_intents() {
        let filters = BTreeMap::from([("status".to_string(), "待办".to_string())]);
        let query_state = QueryState {
            filters: BTreeMap::from([("status".to_string(), "待办".to_string())]),
            search: Some(" host keyword ".to_string()),
            group: Vec::new(),
            time_range: None,
        };
        let filter_intents = vec![FilterIntent {
            dimension: " status ".to_string(),
            operator: FilterOperator::Eq,
            value: " 待办 ".to_string(),
            source: FilterIntentSource::FilterBar,
        }];
        let scope = runtime_metric_eval_scope(
            None,
            "warning_list",
            "home",
            Some("scenes/home.mei"),
            None,
            &filters,
            Some(&query_state),
            &filter_intents,
            "deps=v1",
        )
        .expect("runtime metric eval scope");
        assert_eq!(scope.query_state.filters.get("status"), Some(&"待办".to_string()));
        assert_eq!(scope.query_state.search.as_deref(), Some("host keyword"));
        assert_eq!(scope.filter_intents.len(), 1);
        assert_eq!(scope.filter_intents[0].source, FilterIntentSource::FilterBar);
        assert_eq!(scope.filter_intents[0].dimension, "status");
        assert_eq!(scope.filter_intents[0].value, "待办");
    }

    #[test]
    fn runtime_metric_eval_scope_rejects_unresolved_filter_bindings() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["status".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:warning_list".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: Some(r#"{"normalize":{"原状态":"status"}}"#.to_string()),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let err = runtime_metric_eval_scope(
            Some(&dataset),
            "warning_list",
            "home",
            Some("scenes/home.mei"),
            None,
            &BTreeMap::from([("department".to_string(), "执法".to_string())]),
            None,
            &[],
            "deps=v1",
        )
        .expect_err("unresolved binding should fail");
        assert!(
            err.to_string()
                .contains("requires resolvable filter bindings for dataset `warning_list`: department")
        );
    }

    #[test]
    fn runtime_metric_eval_scope_rejects_unresolved_time_range_binding() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["status".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:warning_list".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: Some(r#"{"normalize":{"原状态":"status"}}"#.to_string()),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let err = runtime_metric_eval_scope(
            Some(&dataset),
            "warning_list",
            "home",
            Some("scenes/home.mei"),
            None,
            &BTreeMap::new(),
            Some(&QueryState {
                filters: BTreeMap::new(),
                search: None,
                group: Vec::new(),
                time_range: Some(QueryTimeRange {
                    dimension: Some("created_at".to_string()),
                    start: Some("2024-01-01".to_string()),
                    end: Some("2024-12-31".to_string()),
                    preset: None,
                }),
            }),
            &[],
            "deps=v1",
        )
        .expect_err("unresolved time range binding should fail");
        assert!(
            err.to_string().contains(
                "requires resolvable time_range.dimension binding for dataset `warning_list`: created_at"
            )
        );
    }

    #[test]
    fn query_state_from_request_prefers_host_supplied_search() {
        let filters = BTreeMap::from([("status".to_string(), "待办".to_string())]);
        let query_state = QueryState {
            filters: BTreeMap::new(),
            search: Some(" host keyword ".to_string()),
            group: Vec::new(),
            time_range: None,
        };
        let merged = query_state_from_request(&filters, Some(" request keyword "), Some(&query_state));
        assert_eq!(merged.filters.get("status"), Some(&"待办".to_string()));
        assert_eq!(merged.search.as_deref(), Some("host keyword"));
    }

    #[test]
    fn query_state_from_request_normalizes_group_and_time_range() {
        let merged = query_state_from_request(
            &BTreeMap::new(),
            None,
            Some(&QueryState {
                filters: BTreeMap::new(),
                search: None,
                group: vec![" park ".to_string(), "park".to_string(), "".to_string()],
                time_range: Some(QueryTimeRange {
                    dimension: Some(" created_at ".to_string()),
                    start: Some(" 2024-01-01 ".to_string()),
                    end: Some(" 2024-12-31 ".to_string()),
                    preset: Some(" year ".to_string()),
                }),
            }),
        );
        assert_eq!(merged.group, vec!["park".to_string()]);
        assert_eq!(
            merged.time_range,
            Some(QueryTimeRange {
                dimension: Some("created_at".to_string()),
                start: Some("2024-01-01".to_string()),
                end: Some("2024-12-31".to_string()),
                preset: Some("year".to_string()),
            })
        );
    }

    #[test]
    fn query_state_from_request_allows_blank_host_search_to_clear_top_level_search() {
        let merged = query_state_from_request(
            &BTreeMap::new(),
            Some("request keyword"),
            Some(&QueryState {
                filters: BTreeMap::new(),
                search: Some("   ".to_string()),
                group: Vec::new(),
                time_range: None,
            }),
        );
        assert_eq!(merged.search, None);
    }

    #[test]
    fn runtime_metric_workset_uses_semantic_closure_for_requested_metrics() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:warning_list".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::from([
                (
                    "sales_total".to_string(),
                    serde_json::json!({
                        "key": "sales_total",
                        "explain": [
                            {
                                "__kind": "data_product",
                                "id": "detail_table",
                                "shape": "dataframe",
                                "value": [{"id": 1}]
                            }
                        ]
                    }),
                ),
                (
                    "sales_total::detail_table".to_string(),
                    serde_json::json!({
                        "key": "sales_total::detail_table",
                        "shape": "dataframe",
                        "value": [{"id": 1}]
                    }),
                ),
            ]),
            runtime_analysis_graph: mei_lang_kernel::build_runtime_analysis_graph(
                &BTreeMap::from([(
                    "sales_total".to_string(),
                    serde_json::json!({
                        "key": "sales_total",
                        "explain": [
                            {
                                "__kind": "data_product",
                                "id": "detail_table",
                                "shape": "dataframe",
                                "value": [{"id": 1}]
                            }
                        ]
                    }),
                )]),
                "warning_list",
            ),
            runtime_analysis_contracts: Default::default(),
        };
        let workset = runtime_metric_workset(
            "warning_list",
            &["sales_total".to_string()],
            &dataset,
        );
        assert_eq!(workset.resolved_metric_ids, vec!["sales_total".to_string()]);
        assert_eq!(
            workset.eval_metric_ids,
            Some(vec![
                "sales_total".to_string(),
                "sales_total::detail_table".to_string(),
            ])
        );
        assert_eq!(workset.defs_for_hydrate.len(), 2);
    }
}
