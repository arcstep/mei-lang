use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_home_ai_warning_cockpit_hydrated_assembly_includes_chart_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw home scene");
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("ai_warning_cockpit_board")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected hydrated ai_warning_cockpit_board assembly on home compile, keys: {:?}",
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
        slots.len() >= 4,
        "home hydrated ai_warning board should include chart+detail+preview slots, got: {slots:?}"
    );
    let components: Vec<&str> = slots
        .iter()
        .filter_map(|slot| {
            slot.as_object()
                .and_then(|map| map.get("component"))
                .and_then(Value::as_str)
        })
        .collect();
    assert!(
        components.iter().filter(|c| **c == "chart").count() >= 2,
        "expected chart slots on home hydrated assembly, got components: {components:?}"
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("home scene contract");
    let encoded = serde_json::to_string(contract).expect("encode home contract");
    assert!(
        encoded.contains("ai_warning_cockpit_board"),
        "home contract should lower ai_warning popup"
    );
    assert!(
        encoded.contains("\"composition_by_source_unit\"")
            && encoded.contains("\"composition_by_warning_type\""),
        "home popup should lower chart projection slots in scene contract"
    );
}

#[test]
fn compile_spbjw_enforcement_personnel_board_preview_precompiles_single_route() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.board.mei";
    let scene_id = "enforcement_personnel_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    assert_eq!(compiled.active_scene.as_deref(), Some(scene_id));
    let routes_attempted = compiled
        .diagnostics
        .iter()
        .find_map(|diag| {
            if diag.code != "route_precompile_stats" {
                return None;
            }
            diag.message.split(',').find_map(|part| {
                part.trim()
                    .strip_prefix("routes_attempted=")
                    .and_then(|value| value.parse::<usize>().ok())
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "route_precompile_stats missing, diagnostics: {:?}",
                compiled.diagnostics
            )
        });
    assert_eq!(
        routes_attempted, 1,
        "multi-export board manage preview should precompile one route, diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key(scene_id),
        "expected assembly for `{scene_id}`, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        !compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "sibling board should not be pre-warmed, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_issue_handling_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/07-问题办理.board.mei";
    let scene_id = "issue_pending_analytics_board";
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
        "issue analytics board export preview assembly should include projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    let encoded = serde_json::to_string(assembly).expect("encode assembly");
    assert!(
        encoded.contains("\"layout_zone\":\"chart\"")
            && encoded.contains("\"layout_zone\":\"detail\""),
        "issue analytics board assembly should lower chart/detail layout_zone, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_mode") && encoded.contains("analytics"),
        "issue analytics board assembly should lower to analytics layout_mode, got: {encoded}"
    );
}

