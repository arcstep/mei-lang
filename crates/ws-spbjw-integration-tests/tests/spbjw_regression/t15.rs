use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_supervision_effectiveness_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    for board_id in [
        "effect_transfer_clue_analytics_board",
        "effect_filing_analytics_board",
        "effect_sanction_analytics_board",
        "effect_handled_analytics_board",
        "effect_recovered_analytics_board",
        "effect_mechanism_documents_board",
    ] {
        assert!(
            encoded.contains(board_id),
            "supervision effectiveness cards should reference analytics board `{board_id}`, got: {encoded}"
        );
    }
    assert!(
        !encoded.contains("generic_drilldown_board"),
        "supervision effectiveness cards should no longer reference generic drilldown shell, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "effectiveness analytics should reference composition explain blocks, got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("effect_transfer_clue_analytics_board"),
        "drilldown context should hydrate transfer clue analytics board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let clue_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_transfer_clue_analytics_board")
        .and_then(Value::as_object)
        .expect("transfer clue analytics assembly");
    let clue_encoded = serde_json::to_string(clue_assembly).expect("encode clue assembly");
    assert!(
        clue_encoded.contains("layout_mode") && clue_encoded.contains("analytics"),
        "transfer clue analytics assembly should lower to analytics layout_mode, got: {clue_encoded}"
    );
    assert!(
        clue_encoded.contains("supervisionDomain") && clue_encoded.contains("month_multi_select"),
        "warning_list effectiveness boards should include warning_list filter fields, got: {clue_encoded}"
    );
    let sanction_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_sanction_analytics_board")
        .and_then(Value::as_object)
        .expect("sanction analytics assembly");
    let sanction_encoded =
        serde_json::to_string(sanction_assembly).expect("encode sanction assembly");
    assert!(
        sanction_encoded.contains("resultId") && sanction_encoded.contains("sanction"),
        "issue_result_list effectiveness boards should include result filter fields, got: {sanction_encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("effect_mechanism_documents_board"),
        "drilldown context should hydrate mechanism documents board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let mechanism_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_mechanism_documents_board")
        .and_then(Value::as_object)
        .expect("mechanism documents assembly");
    let mechanism_encoded =
        serde_json::to_string(mechanism_assembly).expect("encode mechanism documents assembly");
    assert!(
        mechanism_encoded.contains("document_preview"),
        "mechanism documents board should lower document_preview mapping, got: {mechanism_encoded}"
    );
    assert!(
        mechanism_encoded.contains("layout_mode") && mechanism_encoded.contains("list_preview"),
        "mechanism documents board should lower to list_preview layout_mode, got: {mechanism_encoded}"
    );
}

#[test]
fn compile_spbjw_preview_administrative_inspection_park_metrics_succeeds() {
    use ws_spbjw_integration_tests::MetricShape;
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw administrative inspection preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "administrative_inspection preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert!(
        !contract.panels.is_empty(),
        "expected inspection cockpit panels"
    );
    let logistics = compiled
        .resources
        .iter()
        .find(|r| r.id == "logistics_park_vector")
        .and_then(|r| r.dataset.as_ref())
        .expect("logistics_park_vector dataset");
    assert_eq!(
        logistics.rows.len(),
        3,
        "geojson FeatureCollection should yield 3 park rows"
    );
    let inspection_owner = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection_dashboard_ds")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection_dashboard_ds dataset");
    assert!(
        inspection_owner
            .runtime_metric_defs
            .contains_key("park_inspection_total_by_park"),
        "行政检查应内联分园区检查指标"
    );
    let datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let evaluated = evaluate_runtime_metric_defs(
        &inspection_owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["park_inspection_total_by_park".to_string()]),
    )
    .expect("evaluate park_inspection_total_by_park");
    let by_park = evaluated
        .get("park_inspection_total_by_park")
        .expect("park_inspection_total_by_park");
    assert_eq!(by_park.shape, MetricShape::Dataframe);
    let by_park_rows = by_park
        .value
        .as_array()
        .or_else(|| by_park.value.get("value").and_then(|v| v.as_array()))
        .unwrap_or_else(|| panic!("dataframe rows expected, got: {}", by_park.value));
    assert!(
        !by_park_rows.is_empty(),
        "park_inspection_total_by_park should have grouped rows"
    );
    assert!(
        by_park_rows[0]
            .get("园区名称")
            .and_then(|v| v.as_str())
            .is_some(),
        "group_by should use 园区名称 field: {:?}",
        by_park_rows[0]
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_keep_drilldown_object_metadata() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let preview_targets = [
        "scenes/03-指标体系.mei",
        "scenes/07-问题办理.mei",
        "scenes/01-执法要素.mei",
        "scenes/02-行政检查.mei",
        "scenes/04-行政处罚.mei",
        "scenes/05-监督预警.mei",
        "scenes/08-监督成效.mei",
    ];
    let metric_ids = [
        "inspection_frequency_reduction_rate",
        "warnings_verification_rate",
        "effectiveness_verified_rectification_rate",
        "warnings_pending_count",
        "key_enterprises_count",
        "enforcement_items_count",
        "inspections_total_count",
        "inspections_today_count",
        "penalties_today_count",
        "administrative_reconsiderations_count",
        "penalty_revenue_growth_rate",
        "supervision_items_count",
        "effectiveness_transfer_clue_count",
    ];
    for target in preview_targets {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|_| panic!("compile {target} preview"));
        for metric_id in metric_ids {
            let metric = compiled.resources.iter().find_map(|resource| {
                let dataset = resource.dataset.as_ref()?;
                dataset.runtime_metric_defs.get(metric_id)
            });
            if let Some(metric) = metric {
                let meta = metric
                    .get("drilldown_dataset")
                    .or_else(|| metric.get("drilldown"))
                    .or_else(|| metric.get("explain"));
                if let Some(meta) = meta {
                    let is_non_empty = meta
                        .as_object()
                        .map(|value| !value.is_empty())
                        .or_else(|| meta.as_array().map(|value| !value.is_empty()))
                        .unwrap_or(false);
                    assert!(
                        is_non_empty,
                        "{metric_id} should keep non-empty explain/drilldown metadata in {target}"
                    );
                }
            }
        }
    }
}

