use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_cockpit_scenes_use_generic_drilldown_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let cases: [(&str, &str, Vec<&str>); 0] = [];

    for (target, sample_metric_id, legacy_popup_files) in cases {
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
            encoded.contains("drilldown-kit.mei"),
            "`{target}` should reference generic drilldown shell, got: {encoded}"
        );
        assert!(
            encoded.contains("projection_slots"),
            "`{target}` link tabs should lower to projection_slots, got: {encoded}"
        );
        assert!(
            encoded.contains(sample_metric_id),
            "`{target}` projection_slots should include `{sample_metric_id}`, got: {encoded}"
        );
        for legacy in legacy_popup_files {
            assert!(
                !encoded.contains(legacy),
                "`{target}` should not reference legacy popup `{legacy}`, got: {encoded}"
            );
        }
        assert!(
            !encoded.contains("scene_file\":\"../stock/templates/cockpit/drilldown/metric-explain-board.mei\""),
            "`{target}` should not keep direct metric-explain-board scene_file links, got: {encoded}"
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
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
        encoded.contains("enforcement_units_analytics_board"),
        "执法单位卡应引用 analytics board，got: {encoded}"
    );
    assert!(
        encoded.contains("drilldown-kit.mei"),
        "执法对象复合卡仍应保留 generic 多表下钻，got: {encoded}"
    );
    assert!(
        !encoded.contains("enforcement-units-popup.mei"),
        "不应再引用独立 popup scene 文件，got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "drilldown context should hydrate enforcement units analytics assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("执法要素 direct preview should include __world_metrics__");
    let contract = dataset
        .runtime_analysis_contracts
        .get("enforcement_objects_count")
        .and_then(Value::as_object)
        .expect("enforcement_objects_count analysis contract");
    let blocks = contract
        .get("blocks")
        .and_then(Value::as_array)
        .expect("enforcement_objects_count contract blocks");
    assert!(
        blocks.len() >= 5,
        "执法对象 explain 的 5 个 dataframe 应落成 projection blocks，got {blocks:?}"
    );
}

#[test]
fn compile_spbjw_enforcement_units_shell_contract_zones_match_layout() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let board_id = "enforcement_units_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(board_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{board_id}`, keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let shell = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .expect("enforcement units assembly should include shell_contract");
    let zone_ids: Vec<String> = shell
        .get("zones")
        .and_then(Value::as_array)
        .map(|zones| {
            zones
                .iter()
                .filter_map(|zone| zone.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !zone_ids.iter().any(|id| id == "chart"),
        "filter_detail board shell_contract should not include chart zone, got {zone_ids:?}"
    );
    assert!(
        zone_ids.iter().any(|id| id == "filter") && zone_ids.iter().any(|id| id == "detail"),
        "filter_detail board shell_contract should include filter and detail zones, got {zone_ids:?}"
    );
    let areas = shell
        .get("layout")
        .and_then(|layout| layout.get("areas"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .map(|row| {
            row.iter()
                .filter_map(|cell| cell.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert_eq!(
        areas,
        vec!["filter".to_string(), "detail".to_string()],
        "filter_detail board layout areas should be filter+detail only, got {areas:?}"
    );
}
