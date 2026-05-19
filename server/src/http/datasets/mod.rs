//! 数据集懒加载查询、外部文件缓存与分页过滤（按来源类型拆分）。

mod csv_dataset;
mod db_dataset;
mod file_cache;
mod geojson_dataset;
mod json_dataset;
mod metric_dataframe;
mod paginate;
mod paths;
mod query;
mod types;
mod util;
mod xlsx_dataset;
mod xlsx_format;

pub use metric_dataframe::query_metric_dataframe;
pub use query::query_dataset_rows;
pub use types::DatasetQueryOptions;

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
            },
        )
        .expect("query full filtered rows");
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 2);
        assert!(!result.has_more);
        assert_eq!(result.page, 1);
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
            },
            true,
        );
        assert_eq!(result.columns, vec!["id", "source"]);
        assert_eq!(
            result.rows[0].get("id").and_then(|v| v.as_str()),
            Some("1")
        );
    }
}
