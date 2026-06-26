use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw with dataset mei preview");
    let contract = compiled.scene_contract.as_ref().unwrap_or_else(|| {
        panic!(
            "preview should yield scene contract, diagnostics: {:?}",
            compiled.diagnostics
        )
    });
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let path_id = "typical_cases";
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == path_id)
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "expected rows from xlsx for typical_cases, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, mei_lang_kernel::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_routes.iter().any(|r| {
            r.scene_id == "typical_cases" && r.target_file == "scenes/09-监督典型案例.mei"
        }),
        "expected typical_cases in app route registry for access/manage deep links, got: {:?}",
        compiled
            .scene_routes
            .iter()
            .map(|r| (r.scene_id.as_str(), r.target_file.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_select_typical_cases_scene_resolves_dataset_entry() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("typical_cases".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with typical_cases scene (access-style)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/09-监督典型案例.mei"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("typical_cases"));
}

#[test]
fn compile_spbjw_select_enterprise_complaints_scene_resolves_dataset_entry() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("administrative_inspection".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with administrative_inspection scene (discovered route)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/02-行政检查.mei"
    );
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("administrative_inspection")
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enterprise_complaints"),
        "expected enterprise_complaints dataset on administrative_inspection scene"
    );
}

