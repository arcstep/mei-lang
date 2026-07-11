use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_layout_right_typical_cases_popup_lowers_list_preview() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
        },
    )
    .expect("compile layout right preview");
    let encoded = serde_json::to_string(compiled.scene_contract.as_ref().expect("contract"))
        .expect("encode contract");
    assert!(
        encoded.contains("typical_cases") && encoded.contains("typical_cases_detail_board"),
        "layout right should reference typical_cases detail card board, assembly keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_preview_widget_typical_cases_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw typical cases preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/09-监督典型案例.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "typical cases widget preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "typical_cases");
    assert!(
        !contract.panels[0].blocks.is_empty(),
        "typical_cases preview should render blocks"
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        !dataset_resources.is_empty(),
        "typical_cases preview should materialize dataset resources, got: {:?}",
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "typical_cases"),
        "missing typical_cases resource"
    );
    assert!(
        elapsed.as_secs() < 8,
        "widget preview with selective catalog should compile faster than full scan, took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_overview_preview_materializes_imported_metrics() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/layout-左栏.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout left preview errors: {:?}",
        errors
    );
    let ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    let units = compiled
        .resources
        .iter()
        .find(|r| r.id == "enforcement_units")
        .unwrap_or_else(|| panic!("expected enforcement_units in catalog, got {ids:?}"));
    let dataset = units.dataset.as_ref().expect("enforcement_units dataset");
    assert!(
        !dataset.columns.is_empty() && !dataset.rows.is_empty(),
        "imported dataset should at least materialize basic tabular payload"
    );
    assert!(
        dataset.schema.len() >= 1,
        "expected imported dataset schema to be materialized"
    );
}
