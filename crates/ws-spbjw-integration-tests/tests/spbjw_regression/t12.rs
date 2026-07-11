use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_ai_warning_cockpit_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.board.mei";
    let scene_id = "ai_warning_cockpit_board";
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
        slots.len() >= 4,
        "ai_warning list_preview board should lower chart+detail+preview projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
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
        "expected at least 2 chart slots, got components: {components:?}"
    );
    assert!(
        components.iter().any(|c| *c == "data_table"),
        "expected detail table slot, got components: {components:?}"
    );
    assert!(
        components.iter().any(|c| *c == "summary"),
        "expected preview summary slot, got components: {components:?}"
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
    let shell = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .expect("shell_contract");
    let zones = shell
        .get("zones")
        .and_then(Value::as_array)
        .expect("shell zones");
    let chart_zone = zones
        .iter()
        .find(|zone| {
            zone.as_object()
                .and_then(|map| map.get("id"))
                .and_then(Value::as_str)
                == Some("chart")
        })
        .and_then(Value::as_object)
        .expect("chart zone");
    assert_eq!(
        chart_zone.get("parent").and_then(Value::as_str),
        Some("left"),
        "chart zone should stay nested under left container, zones: {zones:?}"
    );
}

#[test]
fn query_spbjw_ai_warning_cockpit_rowset_with_local_board_dataset() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.board.mei";
    let scene_id = "ai_warning_cockpit_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    mei_lang_kernel::locate_dataset_resource(&compiled, "ai_recognition_warnings")
        .expect("board compile should expose ai_recognition_warnings");
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "ai_recognition_warnings",
        "ai_recognition_warnings_count::__scalar_rowset__",
        Some(scene_id),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 5,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("ai warning cockpit rowset should resolve board-local dataset");
    assert!(
        !detail.rows.is_empty(),
        "AI warning cockpit should resolve ai_recognition_warnings in board scene resources"
    );
}

#[test]
fn query_spbjw_inspection_total_analytics_board_resolves_local_dataset() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use mei_lang_kernel::locate_dataset_resource;

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
    locate_dataset_resource(&compiled, "administrative_inspection_dashboard_ds")
        .expect("inspection board compile should expose dashboard dataset");
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "administrative_inspection_dashboard_ds",
        "inspections_total_count::__scalar_rowset__",
        Some(scene_id),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 5,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("inspection total board rowset should resolve local dashboard dataset");
    assert!(
        !detail.rows.is_empty(),
        "inspection total analytics board should resolve dashboard rowset"
    );
}
