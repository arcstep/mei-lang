use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_issue_handling_analytics_projection_slots() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let target = "scenes/07-问题办理.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    for metric_id in [
        "warnings_pending_count",
        "effectiveness_in_progress_count",
        "effectiveness_completed_count",
        "effectiveness_issue_verification_rate",
    ] {
        assert!(
            encoded.contains(metric_id),
            "issue handling scene contract should include metric `{metric_id}`, got: {encoded}"
        );
    }
    for board_id in [
        "issue_pending_analytics_board",
        "issue_doing_analytics_board",
        "issue_done_analytics_board",
        "issue_rate_analytics_board",
    ] {
        assert!(
            encoded.contains(board_id),
            "issue handling status cards should reference analytics board `{board_id}`, got: {encoded}"
        );
    }
    assert!(
        encoded.contains("composition_by_verified"),
        "verification rate card should reference verified status composition explain, got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("issue_rate_analytics_board"),
        "drilldown context should hydrate rate analytics board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let pending_assembly = compiled
        .scene_projection_assembly_by_id
        .get("issue_pending_analytics_board")
        .and_then(Value::as_object)
        .expect("pending analytics assembly");
    let pending_encoded = serde_json::to_string(pending_assembly).expect("encode pending assembly");
    assert!(
        pending_encoded.contains("filter_schema"),
        "issue pending analytics assembly should include filter_schema, got: {pending_encoded}"
    );
    assert!(
        pending_encoded.contains("layout_mode") && pending_encoded.contains("analytics"),
        "issue pending analytics assembly should lower to analytics layout_mode, got: {pending_encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "issue analytics cards should reference composition explain blocks, got: {encoded}"
    );
    assert!(
        encoded.contains("chart_kind") && encoded.contains("column"),
        "issue analytics charts should lower configured chart kinds, got: {encoded}"
    );
    assert!(
        pending_encoded.contains("supervisionDomain")
            && pending_encoded.contains("month_multi_select"),
        "issue analytics boards should include warning_list filter fields, got: {pending_encoded}"
    );
    assert!(
        pending_encoded.contains("\"layout_zone\":\"chart\"")
            && pending_encoded.contains("\"layout_zone\":\"detail\""),
        "issue pending analytics assembly should lower chart/detail layout_zone, got: {pending_encoded}"
    );
    assert!(
        encoded.contains("issue_rate_analytics_board")
            && encoded.contains("warning_detail")
            && encoded.contains("核查情况"),
        "verification rate popup should use analytics board with warning_list detail fields, got: {encoded}"
    );
    let rate_assembly = compiled
        .scene_projection_assembly_by_id
        .get("issue_rate_analytics_board")
        .and_then(Value::as_object)
        .expect("rate analytics assembly");
    let rate_slots = rate_assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_slot = rate_slots.iter().find(|slot| {
        slot.as_object()
            .and_then(|map| map.get("layout_zone"))
            .and_then(Value::as_str)
            == Some("detail")
    });
    assert!(
        detail_slot
            .and_then(Value::as_object)
            .and_then(|slot| slot.get("explain_block_id"))
            .and_then(Value::as_str)
            == Some("detail"),
        "rate analytics detail slot should bind warning_list detail explain, slots: {rate_slots:?}"
    );
}

#[test]
fn compile_spbjw_enterprise_complaints_analytics_board_projection_slots() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let target = "scenes/02-行政检查.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("enterprise_complaints_analytics_board")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "missing enterprise_complaints_analytics_board assembly, keys: {:?}",
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
    let detail_metric_id = slots
        .iter()
        .find(|slot| {
            slot.as_object()
                .and_then(|map| map.get("layout_zone"))
                .and_then(Value::as_str)
                == Some("detail")
        })
        .and_then(Value::as_object)
        .and_then(|slot| slot.get("metric_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .expect("detail slot metric_id");
    assert!(
        detail_metric_id.ends_with("::__scalar_rowset__"),
        "detail slot should bind scalar rowset, got `{detail_metric_id}`"
    );
    let trend_slot = slots
        .iter()
        .find(|slot| {
            slot.as_object()
                .and_then(|map| map.get("explain_block_id"))
                .and_then(Value::as_str)
                == Some("trend_by_report_time")
        })
        .and_then(Value::as_object)
        .expect("trend_by_report_time chart slot");
    assert_eq!(
        trend_slot.get("date_field").and_then(Value::as_str),
        Some("反映时间"),
        "trend slot should carry date_field for filtered client-side aggregation"
    );
}

#[test]
fn compile_spbjw_left_rail_analytics_projection_slots() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    for (target, board_id) in [
        (
            "scenes/01-执法要素.mei",
            "enforcement_units_analytics_board",
        ),
        ("scenes/02-行政检查.mei", "inspection_total_analytics_board"),
        (
            "scenes/03-指标体系.mei",
            "indicator_inspection_frequency_analytics_board",
        ),
        ("scenes/04-行政处罚.mei", "penalty_total_analytics_board"),
    ] {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            ..Default::default()
        },
        )
        .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
        let contract = compiled
            .scene_contract
            .as_ref()
            .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
        let encoded = serde_json::to_string(contract).expect("encode scene contract");
        assert!(
            encoded.contains(board_id),
            "`{target}` should reference analytics board `{board_id}`, got: {encoded}"
        );
        assert!(
            !encoded.contains("generic_drilldown_board") || target == "scenes/01-执法要素.mei",
            "`{target}` should not use generic drilldown except 执法对象，got: {encoded}"
        );
        assert!(
            compiled
                .scene_projection_assembly_by_id
                .contains_key(board_id),
            "`{target}` should hydrate assembly for `{board_id}`, keys: {:?}",
            compiled
                .scene_projection_assembly_by_id
                .keys()
                .collect::<Vec<_>>()
        );
    }
}
