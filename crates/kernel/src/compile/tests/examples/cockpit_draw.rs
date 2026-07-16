use serde_json::Value;

use super::super::super::{compile_app_from_root_with_options, CompileOptions};
use super::super::harness::dev_examples_root;

#[test]
fn compile_cockpit_metric_data_example() {
    let Some(examples) = dev_examples_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let source_root = examples.join("cockpit");
    if !source_root.is_dir() {
        eprintln!("skip: examples/cockpit missing under MEI_TEST_WORKSPACE");
        return;
    }
    let app_root = source_root.join("05-panel");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: Some("metric-data.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile 05-panel/metric-data.mei failed: {error}"));
    assert_eq!(compiled.active_target_file, "metric-data.mei");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "metric-data.mei should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert!(
        compiled.diagnostics.iter().all(|diag| {
            !(diag.code.starts_with("layout_eval_")
                && matches!(diag.severity, crate::Severity::Error))
        }),
        "metric-data.mei should not produce blocking layout eval diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        sc.scene
            .summary
            .as_deref()
            .is_some_and(|value| value.contains("static object") || value.contains("map+patch")),
        "metric-data.mei summary should describe binding demo"
    );
    fn collect_panel_ids(panels: &[crate::UiNodeDecl], out: &mut Vec<String>) {
        for panel in panels {
            out.push(panel.id.clone());
            for node in &panel.blocks {
                if let crate::UiTreeNode::Panel(nested) = node {
                    collect_panel_ids(&[nested.clone()], out);
                }
            }
        }
    }
    let mut panel_ids = Vec::new();
    collect_panel_ids(&sc.panels, &mut panel_ids);
    assert!(panel_ids.iter().any(|id| id == "binding_shell_wide"));
    assert!(panel_ids.iter().any(|id| id == "binding_shell_grid"));
    assert!(panel_ids.iter().any(|id| id == "binding_demo_wide"));
    assert!(panel_ids.iter().any(|id| id == "binding_demo_grid"));
    assert!(panel_ids.iter().any(|id| id == "static_demo_wide_a"));
    let binding_demo = sc
        .panels
        .iter()
        .find_map(|panel| match panel.id.as_str() {
            "binding_shell_grid" => panel.blocks.iter().find_map(|node| match node {
                crate::UiTreeNode::Panel(nested) if nested.id == "binding_demo_grid" => {
                    Some(nested)
                }
                _ => None,
            }),
            _ => None,
        })
        .expect("binding_demo");
    assert_eq!(
        binding_demo
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_auto")
    );
    let grid_areas = binding_demo
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("binding_demo_grid areas");
    assert_eq!(grid_areas[0], ["m0", "m1"]);
    assert_eq!(grid_areas[1], ["m2", "m3"]);
    let binding_demo_wide = sc
        .panels
        .iter()
        .find_map(|panel| match panel.id.as_str() {
            "binding_shell_wide" => panel.blocks.iter().find_map(|node| match node {
                crate::UiTreeNode::Panel(nested) if nested.id == "binding_demo_wide" => {
                    Some(nested)
                }
                _ => None,
            }),
            _ => None,
        })
        .expect("binding_demo_wide");
    let wide_areas = binding_demo_wide
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("binding_demo_wide areas");
    assert_eq!(wide_areas[0], ["m0", "m1", "m2"]);
    assert_eq!(wide_areas[1], ["m3", "m3", "m3"]);
    fn collect_use_keys(nodes: &[crate::UiTreeNode], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                crate::UiTreeNode::Block(block) => out.push(block.use_key.clone()),
                crate::UiTreeNode::Panel(panel) => collect_use_keys(&panel.blocks, out),
                _ => {}
            }
        }
    }
    let mut use_keys = Vec::new();
    for panel in &sc.panels {
        collect_use_keys(&panel.blocks, &mut use_keys);
    }
    let tile_count = use_keys
        .iter()
        .filter(|key| key.as_str() == "cockpit.qunfu-metric-tile")
        .count();
    assert_eq!(
        tile_count, 0,
        "metric-data.mei should not rely on qunfu-metric-tile; got keys: {:?}",
        use_keys
    );
}
