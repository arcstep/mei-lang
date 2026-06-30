use super::super::{
    analysis_closure_metric_ids, build_analysis_graph,
};
use crate::model::{AnalysisEdge, AnalysisGraph, AnalysisNode, SemanticEdgeKind};
use serde_json::json;
use std::collections::BTreeMap;


#[test]
fn analysis_closure_metric_ids_walks_scoped_metric_children() {
    let defs = BTreeMap::from([(
        "sales_total".to_string(),
        json!({
            "key": "sales_total",
            "label": "销售总额",
            "explain": [
                {
                    "__kind": "data_product",
                    "id": "detail_table",
                    "shape": "dataframe",
                    "value": [{"id": "A"}],
                    "explain": [
                        {
                            "__kind": "data_product",
                            "id": "detail_leaf",
                            "shape": "dataframe",
                            "value": [{"id": "leaf"}]
                        }
                    ]
                },
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "source": {"__ref": "metric", "id": "detail_table"}
                }
            ]
        }),
    )]);
    let graph = build_analysis_graph(&defs, "sales_metrics");
    let closure = analysis_closure_metric_ids(&graph, &["sales_total".to_string()]);
    assert_eq!(
        closure,
        vec![
            "sales_total".to_string(),
            "sales_total::detail_table".to_string(),
            "sales_total::detail_table::detail_leaf".to_string(),
        ]
    );
}

#[test]
fn analysis_closure_metric_ids_ignores_narrative_support_nodes() {
    let defs = BTreeMap::from([(
        "sales_total".to_string(),
        json!({
            "key": "sales_total",
            "label": "销售总额",
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "definition",
                    "kind": "definition",
                    "note": "口径说明"
                },
                {
                    "__kind": "data_product",
                    "id": "detail_table",
                    "shape": "dataframe",
                    "value": [{"id": "A"}]
                }
            ]
        }),
    )]);
    let graph = build_analysis_graph(&defs, "sales_metrics");
    let closure = analysis_closure_metric_ids(&graph, &["sales_total".to_string()]);
    assert_eq!(
        closure,
        vec![
            "sales_total".to_string(),
            "sales_total::detail_table".to_string(),
        ]
    );
    assert!(
        !closure.iter().any(|id| id.contains('#')),
        "semantic closure used for runtime metric selection should ignore narrative-only support nodes"
    );
}

#[test]
fn analysis_closure_metric_ids_skips_auxiliary_association_edges() {
    let graph = AnalysisGraph {
        nodes: BTreeMap::from([
            (
                "root".to_string(),
                AnalysisNode {
                    id: "root".to_string(),
                    node_kind: "metric".to_string(),
                    ..Default::default()
                },
            ),
            (
                "related".to_string(),
                AnalysisNode {
                    id: "related".to_string(),
                    node_kind: "metric".to_string(),
                    ..Default::default()
                },
            ),
        ]),
        edges: vec![AnalysisEdge {
            from: "root".to_string(),
            to: "related".to_string(),
            role: "association".to_string(),
            semantic_kind: SemanticEdgeKind::Association,
        }],
    };

    let closure = analysis_closure_metric_ids(&graph, &["root".to_string()]);
    assert_eq!(
        closure,
        vec!["root".to_string()],
        "auxiliary association edges should not silently expand the default semantic closure"
    );
}
