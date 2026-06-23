mod cache_key;
mod query_normalize;
mod workset;

pub(crate) use query_normalize::{
    normalize_query_filters, normalize_query_search, query_state_from_request,
};

pub(crate) use cache_key::{
    dataset_resource_lookup_aliases, eval_node_cache_key,
    metric_dataframe_artifact_lookup_cache_keys, metric_request_revision_fingerprint,
    metric_request_revision_fingerprint_for_compiled, metric_response_artifact_lookup_cache_keys,
    metric_scope_cache_key, runtime_metric_eval_scope, serialize_cache_value,
};
pub(crate) use workset::runtime_metric_workset;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use mei_lang_kernel::{
        CompiledApp, DatasetView, DimensionBinding, FilterIntent, FilterIntentSource, FilterOperator,
        QueryState, QueryTimeRange, RuntimeMetricEvalScope, SourceDecl,
    };
    use serde_json::Value;

    use crate::types::DatasetQueryOptions;

    use super::cache_key::{
        eval_node_cache_key, metric_request_revision_fingerprint,
        metric_response_artifact_lookup_cache_keys, runtime_metric_eval_scope,
    };
    use super::query_normalize::{
        normalize_query_filters, normalize_query_search, query_state_from_request,
    };
    use super::{metric_scope_cache_key, runtime_metric_workset};

    #[test]
    fn metric_response_lookup_prefers_prebuild_keys_in_default_scope() {
        let owner_dataset = DatasetView {
            id: "sample".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:sample".to_string(),
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
        };
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "/tmp/demo".to_string(),
            scene_routes: Vec::new(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![mei_lang_kernel::LoadedResource {
                id: "sample".to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(owner_dataset.clone()),
            }],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let query = DatasetQueryOptions::default();
        let prebuild_first = metric_response_artifact_lookup_cache_keys(
            "demo",
            Path::new("/tmp/demo"),
            &compiled,
            "home",
            Some("scenes/home.mei"),
            "sample",
            &owner_dataset,
            &query,
            "compile-rev",
            &[],
            true,
        );
        assert!(
            prebuild_first[0].starts_with("prebuild|response|"),
            "expected prebuild key first, got {}",
            prebuild_first[0]
        );
        let scoped_first = metric_response_artifact_lookup_cache_keys(
            "demo",
            Path::new("/tmp/demo"),
            &compiled,
            "home",
            Some("scenes/home.mei"),
            "sample",
            &owner_dataset,
            &query,
            "compile-rev",
            &[],
            false,
        );
        assert!(
            scoped_first[0].starts_with("demo|compile="),
            "expected scoped key first, got {}",
            scoped_first[0]
        );
    }

    #[test]
    fn metric_response_lookup_keys_include_bare_dataset_alias() {
        let owner_dataset = DatasetView {
            id: "scenes/09-监督典型案例.mei::typical_cases".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "upload/13.典型案例清单.xlsx".to_string(),
                sheet: None,
                header_row: Some(1),
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
            runtime_metric_defs: BTreeMap::from([(
                "case_count".to_string(),
                Value::Null,
            )]),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let compiled = CompiledApp {
            app_id: "zhifa".to_string(),
            title: "zhifa".to_string(),
            app_root: "/tmp/zhifa".to_string(),
            scene_routes: Vec::new(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![mei_lang_kernel::LoadedResource {
                id: "scenes/09-监督典型案例.mei::typical_cases".to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(owner_dataset.clone()),
            }],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let keys = metric_response_artifact_lookup_cache_keys(
            "zhifa",
            Path::new("/tmp/zhifa"),
            &compiled,
            "home",
            Some("scenes/home.mei"),
            "scenes/09-监督典型案例.mei::typical_cases",
            &owner_dataset,
            &DatasetQueryOptions::default(),
            "compile-rev",
            &[],
            true,
        );
        assert!(
            keys.iter().any(|key| {
                key == "prebuild|response|app=zhifa|dataset=typical_cases|search=|filters={}|group=[]|time_range=null"
            }),
            "expected bare dataset prebuild key, got {keys:?}"
        );
    }

    #[test]
    fn metric_scope_cache_key_sorts_and_dedups() {
        let value = metric_scope_cache_key(&["b".to_string(), "a".to_string(), "b".to_string()]);
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
        assert_eq!(
            normalize_query_search(Some("  abc ")),
            Some("abc".to_string())
        );
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
            &[&dataset],
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
            &[],
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
        assert_eq!(
            scope.query_state.filters.get("status"),
            Some(&"待办".to_string())
        );
        assert_eq!(scope.query_state.search.as_deref(), Some("host keyword"));
        assert_eq!(scope.filter_intents.len(), 1);
        assert_eq!(
            scope.filter_intents[0].source,
            FilterIntentSource::FilterBar
        );
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
            &[&dataset],
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
        assert!(err.to_string().contains("department"));
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
            &[&dataset],
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
        let merged =
            query_state_from_request(&filters, Some(" request keyword "), Some(&query_state));
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
        let workset =
            runtime_metric_workset("warning_list", &["sales_total".to_string()], &dataset);
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
