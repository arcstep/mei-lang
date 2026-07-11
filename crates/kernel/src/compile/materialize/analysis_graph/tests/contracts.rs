use super::super::build_analysis_contracts;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
fn build_analysis_contracts_hoists_composition_dataframe_without_prior_data_product() {
    let defs = BTreeMap::from([(
        "inspections_total_count".to_string(),
        json!({
            "key": "inspections_total_count",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "count",
                    "rowset": {"__ref": "data", "id": "inspection_rows"}
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "composition_by_agency",
                    "kind": "composition",
                    "by": "检查机构",
                    "top_n": 6
                },
                {
                    "__kind": "explain_item",
                    "id": "detail",
                    "kind": "detail",
                    "fields": ["检查机构"]
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "administrative_inspection_dashboard_ds");
    let contract = contracts.get("inspections_total_count").expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let composition = tab_metrics
        .get("composition")
        .and_then(Value::as_object)
        .expect("composition tab");
    assert_eq!(
        composition.get("metric_id").and_then(Value::as_str),
        Some("inspections_total_count::composition_by_agency")
    );
}

#[test]
fn build_analysis_contracts_hoists_trend_dataframe_without_prior_data_product() {
    let defs = BTreeMap::from([(
        "enterprise_complaints_count".to_string(),
        json!({
            "key": "enterprise_complaints_count",
            "values": {
                "value": {
                    "__kind": "analysis_expr",
                    "type": "count",
                    "rowset": {"__ref": "data", "id": "complaint_rows"}
                }
            },
            "explain": [
                {
                    "__kind": "explain_item",
                    "id": "trend_by_report_time",
                    "kind": "trend",
                    "date_field": "反映时间",
                    "grain": "month"
                },
                {
                    "__kind": "explain_item",
                    "id": "composition_by_satisfaction",
                    "kind": "composition",
                    "by": "群众满意度"
                }
            ]
        }),
    )]);
    let contracts = build_analysis_contracts(&defs, "enterprise_complaints");
    let contract = contracts
        .get("enterprise_complaints_count")
        .expect("contract");
    let tab_metrics = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .expect("tab_metrics");
    let trend = tab_metrics
        .get("trend")
        .and_then(Value::as_object)
        .expect("trend tab");
    assert_eq!(
        trend.get("metric_id").and_then(Value::as_str),
        Some("enterprise_complaints_count::trend_by_report_time")
    );
}
