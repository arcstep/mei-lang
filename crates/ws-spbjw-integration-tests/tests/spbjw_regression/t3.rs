use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_preview_widget_elements_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-左栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/layout-左栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "widget elements preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "layout_left");
    assert!(
        !contract.panels.is_empty(),
        "layout left should resolve left_rail panel, got {}",
        contract.panels.len()
    );
    let left_rail = contract
        .panels
        .iter()
        .find(|p| p.id == "left_rail")
        .expect("left_rail panel from layout capsule");
    assert!(
        !left_rail.blocks.is_empty(),
        "left_rail should carry titled_shell + body blocks from panel_ref"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enforcement_units" || r.id == "administrative_inspection"),
        "layout left preview should merge datasets from embedded rail bodies"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|resource| resource.dataset.is_some()),
        "layout left preview needs selective dataset catalog, got ids: {:?}",
        compiled
            .resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        dataset_resources.len() <= 45,
        "manage widget preview should use selective catalog, not full scan (got {}): {:?}",
        dataset_resources.len(),
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        elapsed.as_secs() < 9,
        "manage widget preview should not compile home + full catalog (21 xlsx), took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_preview_layout_center_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-中栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout center preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout center preview errors: {:?}",
        errors
    );
    assert_eq!(compiled.active_target_file, "scenes/layout-中栏.mei");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("layout center preview scene contract");
    assert_eq!(contract.scene.id, "layout_center");
    assert!(
        contract.panels.len() >= 2,
        "layout center should resolve indicator + realtime panel_ref slots, got {}",
        contract.panels.len()
    );
    assert!(
        contract
            .panels
            .iter()
            .any(|p| p.id == "indicator_system_stats" && !p.blocks.is_empty()),
        "indicator_system_stats should carry body blocks"
    );
    assert!(
        contract
            .panels
            .iter()
            .any(|p| p.id == "realtime_warnings_table" && !p.blocks.is_empty()),
        "realtime_warnings_table should carry body blocks"
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_list"),
        "layout center preview should materialize warning_list from realtime body"
    );
}

#[test]
fn compile_spbjw_preview_widget_metrics_system_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
        },
    )
    .expect("compile spbjw supervision warning preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "metrics widget preview errors: {:?}",
        errors
    );
    assert_eq!(compiled.active_target_file, "scenes/05-监督预警.mei");
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_models"),
        "expected warning_models dataset in resources"
    );
}

