//! 在 runtime metric 求值前，为表达式引用的 file-backed dataset 灌入全量行（走 xlsx/file cache）。

mod binding;
mod collect;
mod lookup;

pub(crate) use collect::expand_metric_defs_for_hydrate;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::DatasetView;
use serde_json::Value;

use crate::query::query_dataset_rows;
use crate::types::DatasetQueryOptions;
use crate::util::elapsed_ms;

pub(crate) use binding::{
    dimension_bindings_from_query_state_for_datasets, resolve_dataset_query_bindings_from_state,
    unique_dataset_views, unresolved_filter_dimensions_for_datasets,
};

pub(crate) fn hydrate_file_backed_datasets_for_metric_defs(
    app_root: &Path,
    datasets: &mut BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    query: &DatasetQueryOptions,
) -> Result<BTreeMap<String, u64>> {
    let referenced = collect::collect_dataset_ids_from_values(
        metric_defs.values().cloned().collect::<Vec<_>>().as_slice(),
    );
    let mut perf = BTreeMap::new();
    let hydrate_started = Instant::now();
    let mut hydrated_count = 0u64;
    let mut dropped_filters_total = 0u64;
    let mut unresolved_filters_total = 0u64;
    let mut unresolved_time_range_total = 0u64;
    for dataset_id in referenced {
        let Some(view) = lookup::lookup_dataset_view(datasets, dataset_id.as_str()) else {
            continue;
        };
        if !dataset_needs_runtime_hydration(view) {
            continue;
        }
        let load_started = Instant::now();
        let binding_resolution = binding::compatible_hydrate_binding_resolution(query, view);
        if !binding_resolution.unresolved_filter_dimensions.is_empty() {
            return Err(anyhow!(
                "runtime metric hydrate requires resolvable filter bindings for dataset `{}`: {}",
                view.id,
                binding_resolution.unresolved_filter_dimensions.join(", ")
            ));
        }
        if let Some(dimension) = binding_resolution
            .unresolved_time_range_dimension
            .as_deref()
        {
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
        let dropped_filters = query.filters.len().saturating_sub(load_query.filters.len()) as u64;
        let unresolved_filters = binding_resolution.unresolved_filter_dimensions.len() as u64;
        let unresolved_time_range =
            u64::from(binding_resolution.unresolved_time_range_dimension.is_some());
        dropped_filters_total += dropped_filters;
        unresolved_filters_total += unresolved_filters;
        unresolved_time_range_total += unresolved_time_range;
        let result = query_dataset_rows(app_root, view, load_query)?;
        let load_ms = elapsed_ms(load_started);
        if let Some(entry) = lookup::lookup_dataset_view_mut(datasets, dataset_id.as_str()) {
            entry.rows = result.rows;
            if !result.columns.is_empty() {
                entry.columns = result.columns;
            }
            hydrated_count += 1;
            perf.insert(format!("hydrate_{dataset_id}_ms"), load_ms);
            perf.insert(
                format!("hydrate_{dataset_id}_applied_filters"),
                applied_filters,
            );
            perf.insert(
                format!("hydrate_{dataset_id}_dropped_filters"),
                dropped_filters,
            );
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
    perf.insert(
        "hydrate_datasets_ms".to_string(),
        elapsed_ms(hydrate_started),
    );
    perf.insert("hydrate_datasets_count".to_string(), hydrated_count);
    perf.insert("hydrate_filter_contract_version".to_string(), 1);
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
    collect::collect_dataset_ids_from_values(
        metric_defs.values().cloned().collect::<Vec<_>>().as_slice(),
    )
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
        collect::collect_dataset_ids_from_value(&defs, &mut ids);
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
        let resolution = binding::compatible_hydrate_binding_resolution(&query, &dataset);
        assert_eq!(resolution.mapped_filters.len(), 1);
        assert_eq!(
            resolution.mapped_filters.get("status"),
            Some(&"待办".to_string())
        );
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
        let resolution = binding::resolve_dataset_query_bindings_from_state(
            &mei_lang_kernel::QueryState {
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
        assert_eq!(
            resolution.unresolved_filter_dimensions,
            vec!["unknown".to_string()]
        );
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
        let bindings = binding::dataset_dimension_bindings(&dataset);
        assert!(bindings
            .iter()
            .any(|binding| binding.dimension == "department" && binding.field == "department"));
        assert!(bindings
            .iter()
            .any(|binding| binding.dimension == "status" && binding.field == "status"));
    }
}
