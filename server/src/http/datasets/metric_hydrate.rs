//! 在 runtime metric 求值前，为表达式引用的 file-backed dataset 灌入全量行（走 xlsx/file cache）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{DatasetView, DimensionBinding, QueryState};
use serde_json::Value;

use super::query::query_dataset_rows;
use super::types::{parse_source_meta, DatasetQueryOptions};
use super::util::elapsed_ms;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DatasetQueryBindingResolution {
    pub mapped_filters: BTreeMap<String, String>,
    pub unresolved_filter_dimensions: Vec<String>,
    pub unresolved_time_range_dimension: Option<String>,
}

pub(crate) fn hydrate_file_backed_datasets_for_metric_defs(
    app_root: &Path,
    datasets: &mut BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    query: &DatasetQueryOptions,
) -> Result<BTreeMap<String, u64>> {
    let referenced = collect_dataset_ids_from_values(
        metric_defs.values().cloned().collect::<Vec<_>>().as_slice(),
    );
    let mut perf = BTreeMap::new();
    let hydrate_started = Instant::now();
    let mut hydrated_count = 0u64;
    let mut dropped_filters_total = 0u64;
    let mut unresolved_filters_total = 0u64;
    let mut unresolved_time_range_total = 0u64;
    for dataset_id in referenced {
        let Some(view) = lookup_dataset_view(datasets, dataset_id.as_str()) else {
            continue;
        };
        if !dataset_needs_runtime_hydration(view) {
            continue;
        }
        let load_started = Instant::now();
        let binding_resolution = compatible_hydrate_binding_resolution(query, view);
        if !binding_resolution.unresolved_filter_dimensions.is_empty() {
            return Err(anyhow!(
                "runtime metric hydrate requires resolvable filter bindings for dataset `{}`: {}",
                view.id,
                binding_resolution.unresolved_filter_dimensions.join(", ")
            ));
        }
        if let Some(dimension) = binding_resolution.unresolved_time_range_dimension.as_deref() {
            return Err(anyhow!(
                "runtime metric hydrate requires resolvable time_range.dimension binding for dataset `{}`: {}",
                view.id,
                dimension
            ));
        }
        let hydrate_filters = binding_resolution.mapped_filters;
        let load_query = DatasetQueryOptions {
            page: 1,
            page_size: 0,
            // 避免把主 dataset 的自由搜索错误传播到引用表；本轮先保守禁用。
            search: None,
            filters: hydrate_filters,
            group: Vec::new(),
            time_range: None,
            collect_all: true,
            sort: Vec::new(),
            column_state: None,
            summary: false,
        };
        let applied_filters = load_query.filters.len() as u64;
        let dropped_filters = query
            .filters
            .len()
            .saturating_sub(load_query.filters.len()) as u64;
        let unresolved_filters = binding_resolution.unresolved_filter_dimensions.len() as u64;
        let unresolved_time_range = u64::from(binding_resolution.unresolved_time_range_dimension.is_some());
        dropped_filters_total += dropped_filters;
        unresolved_filters_total += unresolved_filters;
        unresolved_time_range_total += unresolved_time_range;
        let result = query_dataset_rows(app_root, view, load_query)?;
        let load_ms = elapsed_ms(load_started);
        if let Some(entry) = lookup_dataset_view_mut(datasets, dataset_id.as_str()) {
            entry.rows = result.rows;
            if !result.columns.is_empty() {
                entry.columns = result.columns;
            }
            hydrated_count += 1;
            perf.insert(format!("hydrate_{dataset_id}_ms"), load_ms);
            perf.insert(format!("hydrate_{dataset_id}_applied_filters"), applied_filters);
            perf.insert(format!("hydrate_{dataset_id}_dropped_filters"), dropped_filters);
            perf.insert(
                format!("hydrate_{dataset_id}_unresolved_filters"),
                unresolved_filters,
            );
            perf.insert(
                format!("hydrate_{dataset_id}_unresolved_time_range_binding"),
                unresolved_time_range,
            );
            perf.insert(format!("hydrate_{dataset_id}_search_forwarded"), 0);
            if let Some(hit) = result.perf.get("file_cache_hit") {
                perf.insert(format!("hydrate_{dataset_id}_file_cache_hit"), *hit);
            }
        }
    }
    perf.insert("hydrate_datasets_ms".to_string(), elapsed_ms(hydrate_started));
    perf.insert("hydrate_datasets_count".to_string(), hydrated_count);
    perf.insert(
        "hydrate_filter_contract_version".to_string(),
        1,
    );
    perf.insert(
        "hydrate_dropped_filters_total".to_string(),
        dropped_filters_total,
    );
    perf.insert(
        "hydrate_unresolved_filters_total".to_string(),
        unresolved_filters_total,
    );
    perf.insert(
        "hydrate_unresolved_time_range_total".to_string(),
        unresolved_time_range_total,
    );
    Ok(perf)
}

pub(crate) fn collect_dataset_ids_from_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
) -> BTreeSet<String> {
    collect_dataset_ids_from_values(metric_defs.values().cloned().collect::<Vec<_>>().as_slice())
}

fn dataset_needs_runtime_hydration(dataset: &DatasetView) -> bool {
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return false;
    }
    let kind = dataset.source.kind.trim();
    if kind.eq_ignore_ascii_case("derived") || kind.eq_ignore_ascii_case("world_metrics") {
        return false;
    }
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "csv" | "json" | "geojson" | "xlsx" | "xls" | "db" | ""
    ) || path.ends_with(".csv")
        || path.ends_with(".json")
        || path.ends_with(".geojson")
        || path.ends_with(".xlsx")
        || path.ends_with(".xls")
}

fn compatible_hydrate_binding_resolution(
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
        if !seen.insert((normalized_dimension.to_string(), normalized_field.to_string())) {
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
    bindings
}

fn resolve_filter_binding<'a>(
    bindings: &'a [DimensionBinding],
    dimension: &str,
) -> Option<&'a DimensionBinding> {
    let normalized = dimension.trim();
    if normalized.is_empty() {
        return None;
    }
    bindings.iter().find(|binding| binding.dimension == normalized)
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

fn lookup_dataset_view_mut<'a>(
    datasets: &'a mut BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a mut DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    if datasets.contains_key(normalized) {
        return datasets.get_mut(normalized);
    }
    if datasets.contains_key(dataset_id) {
        return datasets.get_mut(dataset_id);
    }
    let key = datasets
        .iter()
        .find_map(|(key, dataset)| {
            (dataset.id == normalized
                || key.ends_with(&format!("::{normalized}"))
                || key.ends_with(&format!("/{normalized}")))
            .then(|| key.clone())
        })?;
    datasets.get_mut(key.as_str())
}

fn collect_dataset_ids_from_values(values: &[Value]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for value in values {
        collect_dataset_ids_from_value(value, &mut ids);
    }
    ids
}

fn collect_dataset_ids_from_value(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_dataset_ids_from_value(item, out);
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(id) = map
                    .get("from_dataset")
                    .or_else(|| map.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.insert(id.to_string());
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                if map.get("type").and_then(Value::as_str) == Some("rows") {
                    if let Some(id) = map.get("dataset").and_then(Value::as_str) {
                        let text = id.trim();
                        if !text.is_empty() {
                            out.insert(
                                text.strip_prefix("dataset.")
                                    .unwrap_or(text)
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            for nested in map.values() {
                collect_dataset_ids_from_value(nested, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::SourceDecl;
    use serde_json::json;

    #[test]
    fn collect_dataset_ids_from_metric_def_tree() {
        let defs = json!({
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "count",
                    "rowset": {
                        "__kind": "analysis_expr",
                        "type": "where",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "first_by",
                            "rowset": {
                                "__kind": "analysis_expr",
                                "type": "rows",
                                "dataset": "warning_list"
                            }
                        }
                    }
                }
            }
        });
        let mut ids = BTreeSet::new();
        collect_dataset_ids_from_value(&defs, &mut ids);
        assert!(ids.contains("warning_list"));
    }

    #[test]
    fn compatible_hydrate_filters_only_keeps_known_logical_columns() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "upload/demo.xlsx".to_string(),
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
        let mut filters = BTreeMap::new();
        filters.insert("status".to_string(), "待办".to_string());
        filters.insert("department".to_string(), "执法".to_string());
        let query = DatasetQueryOptions {
            filters,
            ..DatasetQueryOptions::default()
        };
        let resolution = compatible_hydrate_binding_resolution(&query, &dataset);
        assert_eq!(resolution.mapped_filters.len(), 1);
        assert_eq!(resolution.mapped_filters.get("status"), Some(&"待办".to_string()));
        assert_eq!(resolution.unresolved_filter_dimensions, vec!["department"]);
    }

    #[test]
    fn resolve_dataset_query_bindings_reports_unresolved_dimensions() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["department".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "upload/demo.xlsx".to_string(),
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
        let resolution = resolve_dataset_query_bindings_from_state(
            &QueryState {
                filters: BTreeMap::from([
                    ("status".to_string(), "待办".to_string()),
                    ("unknown".to_string(), "x".to_string()),
                ]),
                search: None,
                group: Vec::new(),
                time_range: Some(mei_lang_kernel::QueryTimeRange {
                    dimension: Some("created_at".to_string()),
                    start: None,
                    end: None,
                    preset: None,
                }),
            },
            &dataset,
        );
        assert_eq!(
            resolution.mapped_filters,
            BTreeMap::from([("status".to_string(), "待办".to_string())])
        );
        assert_eq!(resolution.unresolved_filter_dimensions, vec!["unknown".to_string()]);
        assert_eq!(
            resolution.unresolved_time_range_dimension,
            Some("created_at".to_string())
        );
    }

    #[test]
    fn hydrate_file_backed_datasets_rejects_unresolved_bindings() {
        let mut datasets = BTreeMap::from([(
            "warning_detail".to_string(),
            DatasetView {
                id: "warning_detail".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["status".to_string()],
                rows: Vec::new(),
                source: SourceDecl {
                    kind: "xlsx".to_string(),
                    path: "upload/detail.xlsx".to_string(),
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
            },
        )]);
        let metric_defs = BTreeMap::from([(
            "pending_count".to_string(),
            json!({
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "count",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "rows",
                            "dataset": "warning_detail"
                        }
                    }
                }
            }),
        )]);
        let err = hydrate_file_backed_datasets_for_metric_defs(
            Path::new("/tmp"),
            &mut datasets,
            &metric_defs,
            &DatasetQueryOptions {
                filters: BTreeMap::from([("unknown".to_string(), "x".to_string())]),
                ..DatasetQueryOptions::default()
            },
        )
        .expect_err("unresolved hydrate binding should fail");
        assert!(
            err.to_string()
                .contains("runtime metric hydrate requires resolvable filter bindings for dataset `warning_detail`: unknown")
        );
    }

    #[test]
    fn dataset_dimension_bindings_encode_runtime_filter_dimensions() {
        let dataset = DatasetView {
            id: "warning_list".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["department".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "upload/demo.xlsx".to_string(),
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
        let bindings = dataset_dimension_bindings(&dataset);
        assert!(bindings.iter().any(|binding| binding.dimension == "department" && binding.field == "department"));
        assert!(bindings.iter().any(|binding| binding.dimension == "status" && binding.field == "status"));
    }
}
