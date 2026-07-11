use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_supervision_warning_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/05-监督预警.mei";
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
    assert!(
        encoded.contains("supervision_items_analytics_board"),
        "items card popup should reference analytics board export, got: {encoded}"
    );
    assert!(
        encoded.contains("supervision_models_analytics_board"),
        "models card popup should reference analytics board export, got: {encoded}"
    );
    assert!(
        encoded.contains("warnings_analytics_board"),
        "warnings card popup should reference board export, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_zone"),
        "analytics projection slots should include layout_zone, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "explicit analytics charts should reference explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_warning_level"),
        "items analytics charts should reference warning-level explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_model_type"),
        "models analytics charts should reference model-type explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("chart_kind") && encoded.contains("column"),
        "analytics charts should lower configured chart kinds, got: {encoded}"
    );
    assert!(
        encoded.contains("\"top_n\":6"),
        "warnings analytics chart slots should carry configurable top_n=6, got: {encoded}"
    );
    assert!(
        encoded.contains("filter_schema"),
        "analytics link should include filter_schema, got: {encoded}"
    );
    assert!(
        encoded.contains("\"layout_zone\":\"chart\"")
            && encoded.contains("\"layout_zone\":\"detail\""),
        "analytics board assembly should lower chart/detail layout_zone aligned with scene panels, got: {encoded}"
    );
    assert!(
        encoded.contains("supervisionDomain")
            && encoded.contains("matter")
            && encoded.contains("modelName")
            && encoded.contains("month_multi_select"),
        "three analytics boards should include dataset-specific filter fields, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_mode") && encoded.contains("analytics"),
        "board assembly should lower to analytics layout_mode, got: {encoded}"
    );
    assert!(
        !encoded.contains("generic_drilldown_board"),
        "supervision warning cards should no longer reference generic drilldown shell, got: {encoded}"
    );
}

#[test]
fn compile_spbjw_supervision_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/05-监督预警.board.mei";
    let scene_id = "supervision_items_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        !slots.is_empty(),
        "board export preview assembly should include projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
    let detail_slot = slots.iter().find(|slot| {
        slot.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == "detail")
    });
    assert_eq!(
        detail_slot.and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board detail slot should use rowset dataset, slots: {slots:?}"
    );
    let chart_slot = slots.iter().find(|slot| {
        slot.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == "composition_by_category")
    });
    assert_eq!(
        chart_slot.and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board chart slot should use rowset dataset, slots: {slots:?}"
    );
}

#[test]
fn compile_spbjw_inspection_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.board.mei";
    let scene_id = "inspection_total_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        slots.len() >= 3,
        "cockpit-wrapped inspection board should lower projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
}
