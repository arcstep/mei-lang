use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_access_home_scene_materializes_ops_theme() {
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
    .expect("compile spbjw access home");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("access home scene contract");
    assert_eq!(
        contract.themes.len(),
        1,
        "access route must inject ops theme"
    );
    assert_eq!(
        contract.themes[0].font.get("4").and_then(|v| v.as_str()),
        Some("36px")
    );
    assert_eq!(
        contract.themes[0].font.get("5").and_then(|v| v.as_str()),
        Some("24px"),
        "ops theme should define font level 5 for metric values"
    );
    assert_eq!(
        contract.themes[0]
            .metric_value
            .get("font")
            .and_then(|v| v.as_str()),
        Some("5")
    );
    let left_rail = contract
        .panels
        .iter()
        .find(|p| p.id == "left_rail_float")
        .expect("home access route should include left_rail_float");
    fn find_panel_in_nodes<'a>(
        nodes: &'a [mei_lang_kernel::UiNodeDecl],
        id: &str,
    ) -> Option<&'a mei_lang_kernel::PanelDecl> {
        for node in nodes {
            let mei_lang_kernel::UiNodeDecl::Panel(panel) = node else {
                continue;
            };
            if panel.id == id {
                return Some(panel);
            }
            if let Some(found) = find_panel_in_nodes(&panel.blocks, id) {
                return Some(found);
            }
        }
        None
    }
    let titled = find_panel_in_nodes(&left_rail.blocks, "enforcement_elements_stats")
        .expect("left rail should nest titled_shell panel");
    assert_eq!(
        titled.head_props.get("font_size").and_then(|v| v.as_str()),
        Some("30px"),
        "titled_shell heading font_size should survive panel_ref merge"
    );
    assert!(
        titled
            .head_props
            .get("background")
            .and_then(|bg| bg.get("image"))
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.contains("linear-gradient")),
        "titled_shell title background should survive panel_ref merge"
    );
}

#[test]
fn compile_spbjw_preview_home_scene_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    assert_eq!(compiled.active_target_file, "scenes/home.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(errors.is_empty(), "home preview errors: {:?}", errors);
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "home");
    let frame = contract.frame.as_ref().expect("home should have frame");
    assert!(
        frame.layout.is_some(),
        "home should expose frame grid layout"
    );
    assert!(
        contract.panels.len() >= 6,
        "home should flatten panel_ref slots into scene panels, got {}",
        contract.panels.len()
    );
    assert!(
        contract.panels.iter().any(|panel| panel.area.is_some()),
        "home should keep area bindings for scene grid panels"
    );
    let overview = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats");
    if let Some(panel) = overview {
        assert!(
            !panel.blocks.is_empty(),
            "home panel(base=panel_ref) should inherit blocks from external panel"
        );
    }
    let resource_ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enforcement_units"),
        "home preview catalog should materialize panel_ref datasets, got {resource_ids:?}"
    );
    let viewport = frame
        .props
        .get("viewport")
        .and_then(|value| value.as_object())
        .expect("home frame should declare viewport props");
    assert_eq!(
        viewport.get("design_width").and_then(|v| v.as_i64()),
        Some(1920)
    );
    assert_eq!(
        viewport.get("design_height").and_then(|v| v.as_i64()),
        Some(1080)
    );
    assert_eq!(contract.themes.len(), 1);
    assert_eq!(contract.themes[0].id, "cockpit");
    assert_eq!(
        contract.themes[0].font.get("4").and_then(|v| v.as_str()),
        Some("36px"),
        "ops.themes.cockpit font scale should materialize into scene_contract"
    );
    let frame_image = contract.themes[0]
        .frame
        .get("background")
        .and_then(|bg| bg.get("image"))
        .and_then(|v| v.as_str());
    assert!(
        frame_image.is_some_and(|v| v.contains("bg@3x.png")),
        "frame bg image ops_param should resolve at compile time, got {frame_image:?}"
    );
    let issue_metrics_owner = "__world_metrics__::scenes/07-问题办理.mei::metrics";
    let issue_metrics = compiled
        .resources
        .iter()
        .find(|r| r.id == issue_metrics_owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("home should import 问题办理 world metrics resource");
    assert_eq!(
        mei_lang_kernel::resolve_runtime_metric_def_key(
            issue_metrics_owner,
            "warnings_pending_count::__scalar_rowset__",
            &issue_metrics.runtime_metric_defs,
        )
        .as_deref(),
        Some("scenes/07-问题办理.mei::warnings_pending_count::__scalar_rowset__"),
        "imported capsule metrics should hoist inferred scalar rowset for detail drilldown"
    );
    assert!(
        compiled
            .resources
            .iter()
            .filter_map(|r| r.dataset.as_ref())
            .any(|dataset| !dataset.metrics.is_empty() || !dataset.runtime_metric_defs.is_empty()),
        "home preview should materialize datasets with metrics"
    );
}

#[test]
fn compile_spbjw_preview_main_mei_keeps_inspection_and_penalty_cockpit_metrics() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("main.mei".to_string()),
        },
    )
    .expect("compile spbjw main preview");
    let datasets: Vec<_> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.as_ref())
        .collect();
    assert!(
        !datasets.is_empty(),
        "main preview should materialize datasets from cockpit scenes"
    );
    assert!(
        datasets
            .iter()
            .any(|dataset| !dataset.metrics.is_empty() || !dataset.runtime_metric_defs.is_empty()),
        "main preview should keep resolved metric payloads"
    );
}

