mod dataset_binding;
mod orchestration;
mod serialization;

pub use dataset_binding::normalize_dataset_columns;
pub use orchestration::{query_world_dataset, query_world_dataset_metrics};

pub const RESOURCE_QUERY_SCHEMA_VERSION: &str = "resource-query-v5";

#[cfg(test)]
mod tests {
    use crate::access_query::normalize_dataset_columns;
    use mei_lang_datasets::project_requested_metrics;
    use mei_lang_kernel::{DatasetView, MetricContract, MetricShape, SourceDecl};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn project_requested_metrics_keeps_requested_id_after_canonical_resolution() {
        let runtime_metric_defs = BTreeMap::from([(
            "capsule/overview.mei::sales_total".to_string(),
            json!({"id": "capsule/overview.mei::sales_total"}),
        )]);
        let metrics_map = BTreeMap::from([(
            "capsule/overview.mei::sales_total".to_string(),
            MetricContract {
                id: "capsule/overview.mei::sales_total".to_string(),
                label: Some("Sales Total".to_string()),
                unit: None,
                value_format: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: Vec::new(),
                dataset: None,
                transforms: Vec::new(),
                value: json!(42),
            },
        )]);
        let projected = project_requested_metrics(
            "__world_metrics__::capsule/overview.mei::metrics",
            &["sales_total".to_string()],
            &runtime_metric_defs,
            &metrics_map,
        );
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].id, "sales_total");
        assert_eq!(projected[0].value, json!(42));
    }

    #[test]
    fn normalize_dataset_columns_caps_default_selection() {
        let dataset = DatasetView {
            id: "ds".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: (0..20).map(|i| format!("c{i}")).collect(),
            rows: Vec::new(),
            source: SourceDecl {
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
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        };
        let cols = normalize_dataset_columns(&dataset, None);
        assert_eq!(cols.len(), 10);
    }
}
