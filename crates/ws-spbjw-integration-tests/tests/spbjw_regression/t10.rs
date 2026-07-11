use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_drilldown_kit_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../stock/templates/cockpit/drilldown/drilldown-kit.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile drilldown kit preview `{target}` failed: {error}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "drilldown kit preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("drilldown kit preview should yield scene contract");
    assert_eq!(contract.scene.id, "generic_drilldown_board");
}

#[test]
fn compile_spbjw_generic_drilldown_board_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../stock/templates/cockpit/drilldown/generic-drilldown-board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile generic drilldown preview `{target}` failed: {error}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "generic drilldown preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("generic drilldown preview should yield scene contract");
    assert_eq!(contract.scene.id, "generic_drilldown_board");
}

#[test]
fn compile_spbjw_analytics_drilldown_board_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| {
        panic!("compile analytics drilldown preview `{target}` failed: {error}")
    });
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "analytics drilldown preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("analytics drilldown preview should yield scene contract");
    assert_eq!(contract.scene.id, "analytics_drilldown_board");
}
