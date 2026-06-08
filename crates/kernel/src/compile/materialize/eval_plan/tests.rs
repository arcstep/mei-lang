use super::{build_eval_plan, EvalPlanEdgeKind, EvalPlanNodeKind};
use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;
use crate::model::{DatasetView, QueryState, QueryTimeRange, SourceDecl};
use serde_json::json;
use std::collections::BTreeMap;

fn dataset(id: &str, kind: &str, path: &str) -> DatasetView {
    DatasetView {
        id: id.to_string(),
        title: None,
        purpose: None,
        schema: Vec::new(),
        stage_schema: Vec::new(),
        columns: Vec::new(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: kind.to_string(),
            path: path.to_string(),
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
    }
}

#[test]
fn build_eval_plan_tracks_metric_rowset_scalar_and_hydrate_nodes() {
    let defs = BTreeMap::from([(
        "sales_total".to_string(),
        json!({
            "key": "sales_total",
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
    let datasets = BTreeMap::from([
        (
            "warning_list".to_string(),
            dataset("warning_list", "derived", "dataset_view:warning_list"),
        ),
        (
            "warning_detail".to_string(),
            dataset("warning_detail", "xlsx", "upload/detail.xlsx"),
        ),
    ]);
    let scope = RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState {
            filters: BTreeMap::new(),
            search: None,
            group: vec!["park".to_string()],
            time_range: Some(QueryTimeRange {
                dimension: Some("created_at".to_string()),
                start: Some("2024-01-01".to_string()),
                end: Some("2024-12-31".to_string()),
                preset: Some("year".to_string()),
            }),
        },
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{}".to_string(),
        dependency_revision_key: "deps=v1".to_string(),
    };
    let plan = build_eval_plan(&defs, Some(&["sales_total".to_string()]), &datasets, &scope);
    assert_eq!(plan.targets, vec!["sales_total".to_string()]);
    assert_eq!(plan.scope.group_identity_key, "[\"park\"]");
    assert_eq!(
        plan.scope.time_range_identity_key,
        "{\"dimension\":\"created_at\",\"start\":\"2024-01-01\",\"end\":\"2024-12-31\",\"preset\":\"year\"}"
    );
    assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::MetricEval), 1);
    assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr), 1);
    assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::Rowset), 1);
    assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::Hydrate), 1);
    assert!(plan
        .edges
        .iter()
        .any(|edge| edge.from == "metric:sales_total" && edge.kind == EvalPlanEdgeKind::DependsOn));
}
