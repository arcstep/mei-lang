//! 数据集懒加载查询、外部文件缓存与分页过滤（按来源类型拆分）。

mod csv_dataset;
mod db_dataset;
mod file_cache;
mod json_dataset;
mod paginate;
mod paths;
mod query;
mod types;
mod util;
mod xlsx_dataset;
mod xlsx_format;

pub use types::DatasetQueryOptions;
pub use query::query_dataset_rows;

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
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
        };
        let result = query_dataset_rows(
            PathBuf::from(".").as_path(),
            &dataset,
            DatasetQueryOptions {
                page: 1,
                page_size: 1,
                search: None,
                filters: BTreeMap::new(),
            },
        )
        .expect("query in-memory dataset");
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 1);
        assert!(!result.lazy);
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
                content: Some(
                    json!({
                        "lazy": {"enabled": true, "default_page_size": 2, "max_page_size": 5},
                        "normalize": {},
                    })
                    .to_string(),
                ),
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
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
            },
        )
        .expect("query lazy csv");
        assert!(result.lazy);
        assert_eq!(result.total, 2);
        assert_eq!(result.rows.len(), 2);
    }
}
