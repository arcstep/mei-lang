use mei_lang_kernel::{
    DatasetView, SourceDecl, WorldSemanticExplainBlock, WorldSemanticMetric,
};

use super::lookup::{resolve_explain_block_id, tabular_metric_lookup_candidates};

#[test]
fn tabular_metric_lookup_candidates_resolve_detail_scalar_rowset() {
    let detail = WorldSemanticExplainBlock {
        id: "detail".to_string(),
        kind: "detail".to_string(),
        label: Some("单位明细".to_string()),
        by: None,
        support_role: Some("detail".to_string()),
    };
    let candidates = tabular_metric_lookup_candidates(
        "enforcement_units_count",
        Some("detail"),
        Some(&detail),
        None,
        "__world_metrics__",
    );
    assert!(
        candidates
            .iter()
            .any(|key| key == "enforcement_units_count::__scalar_rowset__"),
        "detail explain should fall back to scalar rowset: {candidates:?}"
    );
}

#[test]
fn tabular_metric_lookup_candidates_prefers_analysis_contract_node_id() {
    use serde_json::json;
    use std::collections::BTreeMap;

    let mut runtime_analysis_contracts = BTreeMap::new();
    runtime_analysis_contracts.insert(
        "enforcement_objects_count".to_string(),
        json!({
            "blocks": [{
                "id": "enforcement_agency_objects_table",
                "node_id": "enforcement_objects_count::enforcement_agency_objects_table",
            }]
        }),
    );
    let dataset = DatasetView {
        id: "__world_metrics__".to_string(),
        title: None,
        purpose: None,
        schema: Vec::new(),
        stage_schema: Vec::new(),
        columns: Vec::new(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: "world_metrics".to_string(),
            path: String::new(),
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
        runtime_analysis_contracts,
    };
    let block = WorldSemanticExplainBlock {
        id: "enforcement_agency_objects_table".to_string(),
        kind: "data_product".to_string(),
        label: None,
        by: None,
        support_role: None,
    };
    let candidates = tabular_metric_lookup_candidates(
        "enforcement_objects_count",
        Some("enforcement_agency_objects_table"),
        Some(&block),
        Some(&dataset),
        "__world_metrics__",
    );
    assert_eq!(
        candidates.first().map(String::as_str),
        Some("enforcement_objects_count::enforcement_agency_objects_table")
    );
}

#[test]
fn tabular_metric_lookup_candidates_use_parent_for_top_level_metric() {
    let candidates = tabular_metric_lookup_candidates(
        "enterprise_map_rows_2025",
        None,
        None,
        None,
        "__world_metrics__",
    );
    assert_eq!(candidates, vec!["enterprise_map_rows_2025".to_string()]);
}

#[test]
fn resolve_explain_block_id_maps_legacy_data_product_index() {
    let metric = WorldSemanticMetric {
        id: "enforcement_objects_count".to_string(),
        label: None,
        unit: None,
        note: None,
        explain: vec![
            WorldSemanticExplainBlock {
                id: "enforcement_venues_table".to_string(),
                kind: "data_product".to_string(),
                label: Some("场所".to_string()),
                by: None,
                support_role: None,
            },
            WorldSemanticExplainBlock {
                id: "enforcement_agency_objects_table".to_string(),
                kind: "data_product".to_string(),
                label: Some("机构对象".to_string()),
                by: None,
                support_role: None,
            },
        ],
    };
    assert_eq!(
        resolve_explain_block_id(&metric, Some("data_product_0")),
        Some("enforcement_venues_table")
    );
    assert_eq!(
        resolve_explain_block_id(&metric, Some("enforcement_agency_objects_table")),
        Some("enforcement_agency_objects_table")
    );
}
