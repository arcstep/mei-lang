use super::{
    AnalysisEdge, AnalysisGraph, AnalysisNode, DatasetView, SemanticEdgeKind, SemanticNodeKind,
};
use crate::model::SourceDecl;
use std::collections::BTreeMap;

fn empty_dataset() -> DatasetView {
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
    }
}

#[test]
fn dataset_view_runtime_metric_helpers_encode_truth_layers() {
    let mut dataset = empty_dataset();
    assert!(!dataset.has_runtime_metric_defs());
    assert!(!dataset.uses_compiled_metric_snapshot_only());

    dataset.metrics.insert(
        "sales_total".to_string(),
        serde_json::from_value(serde_json::json!({
            "id": "sales_total",
            "shape": "scalar",
            "value": {"value": 1}
        }))
        .expect("metric snapshot"),
    );
    assert!(!dataset.has_runtime_metric_defs());
    assert!(dataset.uses_compiled_metric_snapshot_only());

    dataset.runtime_metric_defs.insert(
        "sales_total".to_string(),
        serde_json::json!({"id": "sales_total", "shape": "scalar_map", "values": {"value": 1}}),
    );
    assert!(dataset.has_runtime_metric_defs());
    assert!(
        !dataset.uses_compiled_metric_snapshot_only(),
        "runtime defs should become the runtime-authoritative source when present"
    );
}

#[test]
fn analysis_node_and_edge_encode_semantic_categories() {
    let metric = AnalysisNode {
        node_kind: "metric".to_string(),
        semantic_kind: SemanticNodeKind::Metric,
        ..Default::default()
    };
    let narrative = AnalysisNode {
        node_kind: "narrative".to_string(),
        semantic_kind: SemanticNodeKind::NarrativeSupport,
        ..Default::default()
    };
    let tabular = AnalysisNode {
        node_kind: "tabular_source".to_string(),
        semantic_kind: SemanticNodeKind::TabularSource,
        tabular_source_dataset_id: Some("warning_list".to_string()),
        ..Default::default()
    };
    let support = AnalysisEdge {
        role: "detail".to_string(),
        semantic_kind: SemanticEdgeKind::Support,
        ..Default::default()
    };
    let scope_metric = AnalysisEdge {
        role: "scope_metric".to_string(),
        semantic_kind: SemanticEdgeKind::ScopeMetric,
        ..Default::default()
    };
    let association = AnalysisEdge {
        role: "association".to_string(),
        semantic_kind: SemanticEdgeKind::Association,
        ..Default::default()
    };

    assert_eq!(metric.semantic_kind(), SemanticNodeKind::Metric);
    assert!(metric.participates_in_metric_closure());
    assert_eq!(
        narrative.semantic_kind(),
        SemanticNodeKind::NarrativeSupport
    );
    assert!(!narrative.participates_in_metric_closure());
    assert_eq!(tabular.semantic_kind(), SemanticNodeKind::TabularSource);
    assert!(!tabular.participates_in_metric_closure());

    assert_eq!(support.semantic_kind(), SemanticEdgeKind::Support);
    assert!(support.participates_in_default_closure());
    assert_eq!(scope_metric.semantic_kind(), SemanticEdgeKind::ScopeMetric);
    assert!(scope_metric.participates_in_default_closure());
    assert_eq!(association.semantic_kind(), SemanticEdgeKind::Association);
    assert!(!association.participates_in_default_closure());
}

#[test]
fn analysis_graph_validator_rejects_non_tabular_lineage_targets() {
    let graph = AnalysisGraph {
        nodes: BTreeMap::from([
            (
                "sales_total".to_string(),
                AnalysisNode {
                    id: "sales_total".to_string(),
                    canonical_metric_id: Some("sales_total".to_string()),
                    node_kind: "metric".to_string(),
                    semantic_kind: SemanticNodeKind::Metric,
                    ..Default::default()
                },
            ),
            (
                "detail_note".to_string(),
                AnalysisNode {
                    id: "detail_note".to_string(),
                    node_kind: "narrative".to_string(),
                    semantic_kind: SemanticNodeKind::NarrativeSupport,
                    ..Default::default()
                },
            ),
        ]),
        edges: vec![AnalysisEdge {
            from: "sales_total".to_string(),
            to: "detail_note".to_string(),
            role: "lineage".to_string(),
            semantic_kind: SemanticEdgeKind::Lineage,
        }],
    };
    let errors = graph.validate_invariants();
    assert!(errors
        .iter()
        .any(|item| item.contains("must target a tabular source node")));
}
