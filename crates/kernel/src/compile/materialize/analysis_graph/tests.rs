use super::{analysis_closure_metric_ids, build_analysis_contracts, build_analysis_graph};
use crate::model::{
AnalysisEdge, AnalysisGraph, AnalysisNode, SemanticEdgeKind, SemanticNodeKind,
};
use serde_json::{json, Value};
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
fn build_analysis_contracts_ignores_legacy_explain_object() {
    let defs = BTreeMap::from([(
        "sales_total".to_string(),
        json!({
            "key": "sales_total",
            "label": "销售总额",
            "explain": {
                "note": "legacy explain object",
                "detail_table_metric_id": "detail_table"
            }
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "sales_metrics");
    assert!(
        contracts.get("sales_total").is_none(),
        "legacy explain object should not build analysis contracts"
    );
}

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
fn build_analysis_contracts_emits_blocks_for_explain_dataframe_only() {
    let defs = BTreeMap::from([(
        "objects_total".to_string(),
        json!({
            "key": "objects_total",
            "label": "执法对象",
            "explain": [
                {
                    "__kind": "data_product",
                    "id": "venues_table",
                    "shape": "dataframe",
                    "label": "场所",
                    "value": [{"id": 1}],
                    "schema": [{"id": "id", "type": "integer"}, {"id": "name", "type": "string"}]
                },
                {
                    "__kind": "data_product",
                    "id": "parks_table",
                    "shape": "dataframe",
                    "label": "园区",
                    "value": [{"园区ID": "P1"}],
                    "schema": [{"id": "园区ID", "type": "string"}]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "world_metrics");
    let contract = contracts.get("objects_total").expect("contract");
    let blocks = contract
        .get("blocks")
        .and_then(Value::as_array)
        .expect("blocks");
    assert_eq!(blocks.len(), 2);
    let first = blocks[0].as_object().expect("block");
    assert_eq!(
        first.get("metric_id").and_then(Value::as_str),
        Some("objects_total::venues_table")
    );
    let second = blocks[1].as_object().expect("block");
    assert_eq!(
        second.get("metric_id").and_then(Value::as_str),
        Some("objects_total::parks_table")
    );
}

#[test]
fn build_analysis_contracts_infers_detail_from_explain_scoped_dataframe() {
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
                    "value": [{"id": "A"}]
                },
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["id"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "sales_metrics");
    let contract = contracts.get("sales_total").expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let detail = tab_metrics
        .get("detail")
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("sales_total::detail_table")
    );
}

#[test]
fn build_analysis_contracts_infers_detail_from_metric_value_lineage() {
    let defs = BTreeMap::from([(
        "unit_count".to_string(),
        json!({
            "key": "unit_count",
            "label": "单位数",
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
                    "fields": ["序号", "类别"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts.get("unit_count").expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let detail = tab_metrics
        .get("detail")
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("unit_count::__scalar_rowset__")
    );
    let runtime_ref = detail
        .get("runtime_ref")
        .and_then(Value::as_object)
        .expect("runtime_ref");
    assert_eq!(
        runtime_ref.get("kind").and_then(Value::as_str),
        Some("metric")
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

#[test]
fn build_analysis_contracts_detail_prefers_lineage_when_composition_present() {
    let defs = BTreeMap::from([(
        "verify_rate".to_string(),
        json!({
            "key": "verify_rate",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "percent",
                    "rowset": {"__ref": "data", "id": "warning_list"}
                }
            },
            "explain": [
                {
                    "__kind": "data_product",
                    "id": "breakdown_table",
                    "shape": "dataframe",
                    "value": [{"status": "yes", "value": 1}]
                },
                {
                    "__kind": "explain_item",
                    "id": "composition",
                    "kind": "composition",
                    "by": "status"
                },
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["预警ID"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts.get("verify_rate").expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let detail = tab_metrics
        .get("detail")
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("verify_rate::__scalar_rowset__")
    );
    let composition = tab_metrics
        .get("composition")
        .and_then(Value::as_object)
        .expect("composition tab");
    assert_eq!(
        composition.get("metric_id").and_then(Value::as_str),
        Some("verify_rate::breakdown_table")
    );
}

#[test]
fn build_analysis_contracts_infers_detail_from_scalar_rowset_without_explain_dataframe() {
    let defs = BTreeMap::from([(
        "transfer_clue_count".to_string(),
        json!({
            "key": "transfer_clue_count",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "count",
                    "rowset": {
                        "__kind": "analysis_expr",
                        "type": "first_by",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "where",
                            "rowset": {"__ref": "data", "id": "warning_list"},
                            "predicate": {"__kind": "analysis_expr", "type": "present", "field": "问题跟踪ID"}
                        },
                        "field": "问题跟踪ID"
                    }
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["问题跟踪ID"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts.get("transfer_clue_count").expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let detail = tab_metrics
        .get("detail")
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("transfer_clue_count::__scalar_rowset__")
    );
}

#[test]
fn build_analysis_contracts_infers_recovered_funds_detail_from_sum_number_rowset() {
    let defs = BTreeMap::from([(
        "recovered_funds".to_string(),
        json!({
            "key": "recovered_funds",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "sum",
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "number",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "first_by",
                            "rowset": {"__ref": "data", "id": "issue_result_list"},
                            "field": "处理结果ID"
                        },
                        "field": "挽回资金"
                    }
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["处理结果ID", "挽回资金"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts.get("recovered_funds").expect("contract");
    let detail = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .and_then(|tabs| tabs.get("detail"))
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("recovered_funds::__scalar_rowset__")
    );
}

#[test]
fn build_analysis_contracts_reads_metric_level_note_and_basis_refs() {
    let defs = BTreeMap::from([(
        "handled_person_times".to_string(),
        json!({
            "key": "handled_person_times",
            "label": "处理人数",
            "note": "按处理结果ID去重计处理人数。",
            "basis_refs": ["12.问题处理结果清单.xlsx", "处理结果ID"],
            "values": {"value": 1},
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["处理结果ID"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts
        .get("handled_person_times")
        .and_then(Value::as_object)
        .expect("contract");
    assert_eq!(
        contract.get("note").and_then(Value::as_str),
        Some("按处理结果ID去重计处理人数。")
    );
    assert_eq!(
        contract
            .get("basis_refs")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );
    assert_eq!(
        contract.get("tabs").and_then(Value::as_array).map(|tabs| {
            tabs.iter()
                .filter_map(Value::as_str)
                .any(|tab| tab == "definition")
        }),
        Some(false)
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
