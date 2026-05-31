//! 数据集懒加载查询、外部文件缓存与分页过滤（按来源类型拆分）。

mod csv_dataset;
mod db_dataset;
mod file_cache;
mod geojson_dataset;
mod json_dataset;
mod metric_cache_key;
mod metric_hydrate;
mod metric_dataframe;
mod metric_locate;
mod paginate;
mod paths;
mod query;
pub mod table_contract;
mod types;
mod util;
mod xlsx_dataset;

pub use metric_dataframe::query_metric_dataframe;
pub(crate) use metric_cache_key::{
    eval_node_cache_key, metric_request_revision_fingerprint, metric_scope_cache_key,
    normalize_query_filters, normalize_query_search, query_state_from_request, runtime_metric_eval_scope,
    runtime_metric_workset, serialize_cache_value,
};
pub(crate) use metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
pub(crate) use metric_locate::{
    locate_runtime_metric_resource, metric_ids_visible_for_dataset, plan_access_metric_eval_for_ids,
};
pub(crate) use file_cache::clear_external_file_cache_for_app;
pub(crate) use metric_dataframe::clear_metric_dataframe_result_cache;
pub use query::query_dataset_rows;
pub use types::{DatasetQueryOptions, TableColumnMeta, TableSummary};

#[cfg(test)]
mod tests {
    use super::{query_dataset_rows, DatasetQueryOptions};
    use mei_lang_kernel::{DatasetView, SourceDecl};
    use serde_json::json;
    use std::{collections::BTreeMap, fs, path::PathBuf};

    #[test]
    fn query_in_memory_dataset_without_lazy() {
        let dataset = DatasetView {
            id: "sample".to_string(),
            title: Some("Sample".to_string()),
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["name".to_string()],
            rows: vec![json!({"name": "A"}), json!({"name": "B"})],
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
        let result = query_dataset_rows(
            PathBuf::from(".").as_path(),
            &dataset,
            DatasetQueryOptions {
                page: 1,
                page_size: 1,
                search: None,
                filters: BTreeMap::new(),
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
        )
        .expect("query in-memory dataset");
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 1);
        assert!(!result.lazy);
    }

    /// 未在元数据写开关时，csv 等外部文件仍走查询路径
    #[test]
    fn query_csv_file_backed_defaults_to_lazy_without_lazy_flag() {
        let root = std::env::temp_dir().join("mei-dataset-query-default-lazy");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        let csv_path = root.join("rows.csv");
        fs::write(&csv_path, "name,city\nalice,chongqing\nbob,beijing\n").expect("write csv");
        let dataset = DatasetView {
            id: "rows".to_string(),
            title: Some("Rows".to_string()),
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["name".to_string(), "city".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "csv".to_string(),
                path: "rows.csv".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: Some(r#"{"normalize":{}}"#.to_string()),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let result = query_dataset_rows(
            &root,
            &dataset,
            DatasetQueryOptions {
                page: 1,
                page_size: 10,
                search: None,
                filters: BTreeMap::new(),
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
        )
        .expect("query csv default lazy");
        assert!(result.lazy);
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn query_csv_dataset_with_lazy_filters() {
        let root = std::env::temp_dir().join("mei-dataset-query-test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        let csv_path = root.join("rows.csv");
        fs::write(
            &csv_path,
            "name,city\nalice,chongqing\nbob,beijing\ncarol,chongqing\n",
        )
        .expect("write csv");
        let dataset = DatasetView {
            id: "rows".to_string(),
            title: Some("Rows".to_string()),
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["name".to_string(), "city".to_string()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "csv".to_string(),
                path: "rows.csv".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: Some(
                    json!({
                        "lazy": {"default_page_size": 2, "max_page_size": 5},
                        "normalize": {},
                    })
                    .to_string(),
                ),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let mut filters = BTreeMap::new();
        filters.insert("city".to_string(), "chongqing".to_string());
        let result = query_dataset_rows(
            &root,
            &dataset,
            DatasetQueryOptions {
                page: 1,
                page_size: 10,
                search: None,
                filters,
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
        )
        .expect("query lazy csv");
        assert!(result.lazy);
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn query_in_memory_dataset_collect_all_returns_full_filtered_rows() {
        let dataset = DatasetView {
            id: "sample".to_string(),
            title: Some("Sample".to_string()),
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["name".to_string(), "city".to_string()],
            rows: vec![
                json!({"name": "alice", "city": "chongqing"}),
                json!({"name": "bob", "city": "beijing"}),
                json!({"name": "carol", "city": "chongqing"}),
            ],
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
        let mut filters = BTreeMap::new();
        filters.insert("city".to_string(), "chongqing".to_string());
        let result = query_dataset_rows(
            PathBuf::from(".").as_path(),
            &dataset,
            DatasetQueryOptions {
                page: 1,
                page_size: 1,
                search: None,
                filters,
                collect_all: true,
                ..DatasetQueryOptions::default()
            },
        )
        .expect("query full filtered rows");
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 2);
        assert!(!result.has_more);
        assert_eq!(result.page, 1);
    }

    #[test]
    fn paginate_rows_sorts_before_paging() {
        use super::paginate::paginate_rows;
        use super::table_contract::TableSortSpec;
        use serde_json::json;

        let rows = vec![
            json!({"name": "bob"}),
            json!({"name": "alice"}),
            json!({"name": "carol"}),
        ];
        let result = paginate_rows(
            rows,
            &["name".to_string()],
            &BTreeMap::new(),
            &DatasetQueryOptions {
                page: 1,
                page_size: 2,
                search: None,
                filters: BTreeMap::new(),
                group: Vec::new(),
                time_range: None,
                collect_all: false,
                sort: vec![TableSortSpec {
                    field: "name".to_string(),
                    direction: "asc".to_string(),
                }],
                column_state: None,
                summary: false,
            },
            false,
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].get("name").and_then(|v| v.as_str()), Some("alice"));
        assert_eq!(result.rows[1].get("name").and_then(|v| v.as_str()), Some("bob"));
        assert!(result.has_more);
    }

    #[test]
    fn paginate_rows_sorts_numeric_strings_as_numbers() {
        use super::paginate::paginate_rows;
        use super::table_contract::TableSortSpec;
        use serde_json::json;

        let rows = vec![
            json!({"amount": "99"}),
            json!({"amount": "920"}),
            json!({"amount": "125"}),
        ];
        let result = paginate_rows(
            rows,
            &["amount".to_string()],
            &BTreeMap::new(),
            &DatasetQueryOptions {
                page: 1,
                page_size: 3,
                search: None,
                filters: BTreeMap::new(),
                group: Vec::new(),
                time_range: None,
                collect_all: false,
                sort: vec![TableSortSpec {
                    field: "amount".to_string(),
                    direction: "desc".to_string(),
                }],
                column_state: None,
                summary: false,
            },
            true,
        );
        assert_eq!(result.rows[0].get("amount").and_then(|v| v.as_str()), Some("920"));
        assert_eq!(result.rows[1].get("amount").and_then(|v| v.as_str()), Some("125"));
        assert_eq!(result.rows[2].get("amount").and_then(|v| v.as_str()), Some("99"));
    }

    #[test]
    fn paginate_rows_returns_logical_columns_when_normalize_maps_source_headers() {
        use super::paginate::paginate_rows;
        use serde_json::json;

        let mut normalize = BTreeMap::new();
        normalize.insert("流水号".to_string(), "id".to_string());
        normalize.insert("反映来源".to_string(), "source".to_string());
        let rows = vec![json!({"流水号": "1", "反映来源": "热线"})];
        let result = paginate_rows(
            rows,
            &["流水号".to_string(), "反映来源".to_string()],
            &normalize,
            &DatasetQueryOptions {
                page: 1,
                page_size: 10,
                search: None,
                filters: BTreeMap::new(),
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
            true,
        );
        assert_eq!(result.columns, vec!["id", "source"]);
        assert_eq!(result.rows[0].get("id").and_then(|v| v.as_str()), Some("1"));
    }
}
