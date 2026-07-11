use super::super::{build_analysis_contracts, expand_runtime_metric_defs};
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
fn expand_runtime_metric_defs_hoists_scalar_rowset_for_ratio() {
    let shared_rowset = json!({
        "__kind": "analysis_expr",
        "type": "where",
        "rowset": {"__ref": "data", "id": "warning_list"},
        "predicate": {
            "__kind": "analysis_expr",
            "type": "in_values",
            "field": "是否查实",
            "values": ["是", "否"]
        }
    });
    let defs = BTreeMap::from([(
        "effectiveness_issue_verification_rate".to_string(),
        json!({
            "key": "effectiveness_issue_verification_rate",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "ratio",
                    "numerator": {
                        "__kind": "analysis_expr",
                        "type": "sum",
                        "value": {
                            "__kind": "analysis_expr",
                            "type": "number",
                            "rowset": shared_rowset.clone(),
                            "field": "查实条数"
                        }
                    },
                    "denominator": {
                        "__kind": "analysis_expr",
                        "type": "sum",
                        "value": {
                            "__kind": "analysis_expr",
                            "type": "number",
                            "rowset": shared_rowset,
                            "field": "预警条数"
                        }
                    }
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "composition_by_verified",
                    "kind": "composition",
                    "by": "是否查实"
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
    let expanded = expand_runtime_metric_defs(&defs);
    assert!(
        expanded.contains_key("effectiveness_issue_verification_rate::__scalar_rowset__"),
        "ratio metrics with shared rowset operands should hoist inferred scalar rowset, keys: {:?}",
        expanded.keys().collect::<Vec<_>>()
    );
    let contracts = build_analysis_contracts(&defs, "__world_metrics__");
    let contract = contracts
        .get("effectiveness_issue_verification_rate")
        .expect("contract");
    let detail = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .and_then(|tabs| tabs.get("detail"))
        .and_then(Value::as_object)
        .expect("detail tab");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("effectiveness_issue_verification_rate::__scalar_rowset__")
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
