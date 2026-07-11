use super::super::build_analysis_graph;
use crate::model::{SemanticEdgeKind, SemanticNodeKind};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn build_analysis_graph_emits_tabular_sources_and_lineage_edges() {
    let defs = BTreeMap::from([(
        "sales_total".to_string(),
        json!({
            "key": "sales_total",
            "label": "销售总额",
            "dataset": "warning_list",
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "source": {"__ref": "data", "from_dataset": "warning_detail"}
                }
            ]
        }),
    )]);
    let graph = build_analysis_graph(&defs, "sales_metrics");
    let root_tabular = graph
        .nodes
        .get("tabular::warning_list")
        .expect("root dataset should materialize as tabular source");
    assert_eq!(
        root_tabular.semantic_kind(),
        SemanticNodeKind::TabularSource
    );
    assert_eq!(
        root_tabular.tabular_source_dataset_id.as_deref(),
        Some("warning_list")
    );
    let detail_tabular = graph
        .nodes
        .get("tabular::warning_detail")
        .expect("detail dataset should materialize as tabular source");
    assert_eq!(
        detail_tabular.semantic_kind(),
        SemanticNodeKind::TabularSource
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.from == "sales_total"
            && edge.to == "tabular::warning_list"
            && edge.semantic_kind() == SemanticEdgeKind::Lineage
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.from == "sales_total"
            && edge.to == "tabular::warning_detail"
            && edge.semantic_kind() == SemanticEdgeKind::Lineage
    }));
    assert!(
        graph.validate_invariants().is_empty(),
        "tabular lineage graph should satisfy semantic invariants"
    );
}

#[test]
fn build_analysis_graph_adds_lineage_from_metric_values() {
    let defs = BTreeMap::from([(
        "unit_count".to_string(),
        json!({
            "key": "unit_count",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "count",
                    "rowset": {"__ref": "data", "id": "enforcement_units"}
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["序号"]
                }
            ]
        }),
    )]);
    let graph = build_analysis_graph(&defs, "__world_metrics__");
    assert!(graph.edges.iter().any(|edge| {
        edge.from == "unit_count"
            && edge.to == "tabular::enforcement_units"
            && edge.semantic_kind() == SemanticEdgeKind::Lineage
    }));
}
