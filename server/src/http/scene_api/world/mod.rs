mod assets;
mod bundle;
mod dataset_llm;
mod inventory;
mod json_shrink;
mod runtime_peek;
mod snapshot;
mod summaries;
mod util;

pub(crate) use assets::{query_world_asset, query_world_assets};
pub(crate) use dataset_llm::{query_world_dataset, query_world_dataset_metrics};
pub(crate) use runtime_peek::query_world_runtime;
pub(crate) use snapshot::{build_world_context_snapshot, build_world_context_snapshot_cached};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mei_lang_kernel::DatasetView;
    use serde_json::{json, Value};

    use super::dataset_llm::{
        normalize_dataset_columns, project_dataset_row, DATASET_QUERY_MAX_CELL_CHARS,
    };
    use super::inventory::{extract_ref_tokens_from_source, related_to_target};
    use super::summaries::summarize_resource_decl;
    use super::util::app_relative_mei_for_preview;

    #[test]
    fn extract_ref_tokens_collects_common_refs() {
        let source = r#"
scene(kind="scene", id="s1", world=world_file_ref(path="worlds/s1-world.mei"))
panel_ref("overview")
metric_ref("sales_growth")
"#;
        let refs = extract_ref_tokens_from_source(source);
        assert!(refs.contains(&"world_file_ref".to_string()));
        assert!(refs.contains(&"panel_ref".to_string()));
        assert!(refs.contains(&"metric_ref".to_string()));
    }

    #[test]
    fn related_target_normalizes_relative_prefix() {
        assert!(related_to_target(
            Some("./apps/demo/main.mei"),
            Some("apps/demo/main.mei")
        ));
        assert!(!related_to_target(
            Some("apps/demo/other.mei"),
            Some("apps/demo/main.mei")
        ));
    }

    #[test]
    fn resource_summary_includes_column_preview() {
        use mei_lang_kernel::{ResourceDecl, SourceDecl};

        let dataset = json!({
            "key": "ds1",
            "kind": "dataframe",
            "columns": [
                {"name": "a", "type": "string", "optional": false},
                {"name": "b", "type": "number", "source": "B"},
            ],
            "normalize": {}
        });
        let item = ResourceDecl {
            id: "ds1".into(),
            kind: "dataset".into(),
            title: None,
            purpose: None,
            source: Some(SourceDecl {
                kind: "xlsx".into(),
                path: "data/x.xlsx".into(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            }),
            content: None,
            dataset: Some(dataset),
            metrics: None,
            filters: None,
        };
        let v = summarize_resource_decl(&item);
        let preview = v
            .pointer("/dataset/schema/columns_preview")
            .expect("columns_preview");
        let arr = preview.as_array().expect("array");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn app_relative_mei_strips_workspace_app_prefix() {
        assert_eq!(
            app_relative_mei_for_preview("spbjw", "spbjw/data/dataset/x.mei").as_deref(),
            Some("data/dataset/x.mei")
        );
        assert_eq!(
            app_relative_mei_for_preview("spbjw", "data/dataset/x.mei").as_deref(),
            Some("data/dataset/x.mei")
        );
        assert_eq!(app_relative_mei_for_preview("spbjw", "data/x.txt"), None);
    }

    #[test]
    fn resource_get_summary_omits_huge_dataset_blob() {
        use mei_lang_kernel::{ResourceDecl, SourceDecl};

        let huge_rows: Value = json!((0..4000).map(|i| json!({"id": i})).collect::<Vec<_>>());
        let huge = json!({ "kind": "tabular", "rows": huge_rows });
        let item = ResourceDecl {
            id: "ds1".into(),
            kind: "dataset".into(),
            title: Some("Demo".into()),
            purpose: None,
            source: Some(SourceDecl {
                kind: "xlsx".into(),
                path: "data/raw/x.xlsx".into(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            }),
            content: None,
            dataset: Some(huge),
            metrics: None,
            filters: None,
        };
        let v = summarize_resource_decl(&item);
        let s = serde_json::to_string(&v).expect("json");
        assert!(
            s.len() < 4_000,
            "summary unexpectedly large: {} chars",
            s.len()
        );
        assert!(
            s.contains("approx_decl_chars"),
            "expected size metadata in summary: {s}"
        );
        assert!(
            !s.contains("\"id\":3999"),
            "expected row bodies not to be inlined: {s}"
        );
    }

    #[test]
    fn dataset_query_default_columns_cap_to_ten() {
        let dataset = DatasetView {
            id: "ds".to_string(),
            title: None,
            purpose: None,
            schema: (0..20)
                .map(|i| mei_lang_kernel::ColumnSchema {
                    name: format!("c{i}"),
                    type_name: "string".to_string(),
                    source: None,
                    optional: false,
                    unit: None,
                })
                .collect(),
            stage_schema: Vec::new(),
            columns: (0..20).map(|i| format!("c{i}")).collect(),
            rows: Vec::new(),
            source: mei_lang_kernel::SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:ds".to_string(),
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
        let cols = normalize_dataset_columns(&dataset, None);
        assert_eq!(cols.len(), 10);
        assert_eq!(cols.first().map(String::as_str), Some("c0"));
        assert_eq!(cols.last().map(String::as_str), Some("c9"));
    }

    #[test]
    fn dataset_row_projection_truncates_long_text() {
        let row = json!({
            "name": "alice",
            "long_text": "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        });
        let mut truncated = 0usize;
        let out = project_dataset_row(
            &row,
            &["name".to_string(), "long_text".to_string()],
            &mut truncated,
        );
        assert_eq!(out.pointer("/name").and_then(Value::as_str), Some("alice"));
        let long = out
            .pointer("/long_text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(long.chars().count() <= DATASET_QUERY_MAX_CELL_CHARS + 1);
        assert!(truncated >= 1);
    }
}
