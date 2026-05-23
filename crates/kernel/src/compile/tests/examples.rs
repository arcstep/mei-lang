use std::fs;

use serde_json::Value;

use super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::harness::{build_regression_workspace_root, workspace_root};
use crate::evaluate_mei_file;

#[test]
fn compile_examples_regressions() {
    let examples = build_regression_workspace_root();
    for app_id in [
        "ds-01-dataset-baseline",
        "cockpit-01-composition-shell",
        "cockpit-02-multi-entry",
        "sim-01-fire-baseline",
        "chart-01-echarts",
        "sim-02-fire-minimal",
        "sim-03-fire-spread",
        "sim-04-fire-multiroom",
    ] {
        let app_root = examples.join("regression-suite").join(app_id);
        let compiled = compile_app_from_root(&examples, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should contain scene contract"
        );
    }
    let _ = fs::remove_dir_all(&examples);
}

#[test]
fn compile_core_examples_baselines() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/core");
    for app_id in [
        "01-single-file-doc",
        "02-external-scene-file",
        "03-multi-panel-baseline",
    ] {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should produce a scene contract"
        );
    }
}

#[test]
fn compile_sim_examples_baselines() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/sim");
    for app_id in [
        "01-fire-baseline",
        "02-fire-minimal",
        "03-fire-spread",
        "04-fire-multiroom",
    ] {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should produce a scene contract"
        );
    }
}

#[test]
fn compile_workspaces_spbjw_baseline() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile spbjw failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "spbjw should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "spbjw should produce scene contract"
    );
}

#[test]
fn compile_core_invalid_examples_report_expected_errors() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/core/_invalid");
    let cases = [
        ("01-multiple-apps", "multiple_apps"),
        ("02-multiple-scenes", "multiple_scenes"),
        ("03-multiple-worlds", "multiple_worlds"),
        ("04-multiple-frames", "multiple_frames"),
        ("05-scene-missing-world", "missing_world"),
        ("06-scene-missing-frame", "missing_frame"),
        ("07-app-missing-scene", "missing_app_scene"),
        (
            "08-scene-external-world-without-world_file_ref",
            "missing_bound_world",
        ),
        (
            "09-scene-external-frame-without-frame_file_ref",
            "missing_bound_frame",
        ),
        (
            "10-world-mutation-before-world-decl",
            "world_mutation_before_world_decl",
        ),
        ("11-world-before-scene-decl", "world_before_scene_decl"),
    ];

    for (app_id, expected_code) in cases {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|diag| diag.code == expected_code
                    && matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should report `{expected_code}`"
        );
    }
}

#[test]
fn compile_refs_examples_baselines() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/refs");
    for app_id in [
        "01-local-ids-in-props",
        "02-metric-from-local-dataset",
        "03-world-imports-external-resources",
        "04-panel-ref-implicit-world",
        "05-local-overrides-external-ledger",
        "06-singleton-base-clone",
        "07-world-collection-base-clone",
        "08-component-base-clone",
        "09-nine-grid-panel-clone",
    ] {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "refs example {app_id} should not produce error diagnostics: {:?}",
            compiled.diagnostics
        );
        assert!(
            compiled.scene_contract.is_some(),
            "refs example {app_id} should produce a scene contract"
        );
    }
}

#[test]
fn compile_refs_invalid_examples_report_expected_errors() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/refs/_invalid");
    let cases = [
        (
            "01-props-external-dataset",
            "external_ref_requires_world_import",
        ),
        ("02-props-misused-world-ref", "misused_world_ref_in_props"),
        ("03-top-level-panel-ref-embed", "panel_ref_embed_removed"),
    ];

    for (app_id, expected_code) in cases {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled.diagnostics.iter().any(|diag| {
                diag.code == expected_code && matches!(diag.severity, crate::Severity::Error)
            }),
            "refs invalid example {app_id} should report `{expected_code}`: {:?}",
            compiled.diagnostics
        );
    }
}

#[test]
fn compile_cockpit_qunfu_chrome_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("03-spbjw-qunfu-chrome");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile 03-spbjw-qunfu-chrome failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "03-spbjw-qunfu-chrome should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "03-spbjw-qunfu-chrome should produce a scene contract"
    );
}

#[test]
fn compile_capability_examples_baselines() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/capability");
    for app_id in ["01-file-query"] {
        let app_root = source_root.join(app_id);
        let compiled = compile_app_from_root(&source_root, &app_root)
            .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
        assert!(
            compiled
                .diagnostics
                .iter()
                .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
            "example {app_id} should not produce error diagnostics"
        );
        assert!(
            compiled.scene_contract.is_some(),
            "example {app_id} should produce a scene contract"
        );
    }
}

#[test]
fn parse_cockpit_default_compare_scene_file() {
    let root = build_regression_workspace_root();
    let path = root.join("regression-suite/cockpit-02-multi-entry/default.mei");
    let value = evaluate_mei_file(&path).expect("parse default compare scene");
    let values = value.as_array().expect("scene file exports array");
    assert!(
        values
            .iter()
            .any(|item| item.get("kind").and_then(|value| value.as_str()) == Some("scene")),
        "default.mei should declare a scene"
    );
    assert!(
        values
            .iter()
            .any(|item| item.get("kind").and_then(|value| value.as_str()) == Some("frame")),
        "default.mei should declare a frame"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_cockpit_header_title_draw_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("04-header-title-draw");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile 04-header-title-draw failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "04-header-title-draw should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "04-header-title-draw should produce a scene contract"
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert!(
        sc.panels.iter().any(|p| p.id == "gallery"),
        "gallery panel must compile; got ids: {:?}",
        sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
}

#[test]
fn compile_cockpit_section_panel_draw_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("05-section-panel-draw");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile 05-section-panel-draw failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "05-section-panel-draw should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "05-section-panel-draw should produce a scene contract"
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    for panel_id in [
        "block_title_metrics",
        "block_title_only",
        "block_metrics_only",
        "block_title_metrics_solid",
        "block_title_metrics_waist",
    ] {
        assert!(
            sc.panels.iter().any(|p| p.id == panel_id),
            "panel {panel_id} must compile; got ids: {:?}",
            sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
    }
    let metrics_only = sc
        .panels
        .iter()
        .find(|panel| panel.id == "block_metrics_only")
        .expect("block_metrics_only");
    let areas = metrics_only
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("metrics_only layout");
    assert_eq!(areas[0], ["m0", "m1", "m2"]);
    assert_eq!(
        metrics_only
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_strip")
    );
}

#[test]
fn compile_cockpit_panel_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("05-panel");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile 05-panel failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "05-panel should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.diagnostics.iter().all(|diag| {
            diag.code != "layout_audit_row_budget_overflow"
                && diag.code != "layout_audit_column_budget_overflow"
        }),
        "05-panel should not trigger fixed-track overflow audit: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "05-panel should produce a scene contract"
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert!(
        sc.panels.iter().any(|p| p.id == "block_title_metrics_bg"),
        "panel block_title_metrics_bg must compile; got ids: {:?}",
        sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    let metrics = sc
        .panels
        .iter()
        .find(|p| p.id == "block_title_metrics_bg")
        .expect("block_title_metrics_bg");
    let body_shell = metrics
        .blocks
        .iter()
        .find_map(|node| match node {
            crate::UiNodeDecl::Panel(panel) if panel.id == "metrics_body" => Some(panel),
            _ => None,
        })
        .expect("block_title_metrics_bg should nest metrics_body_panel");
    let areas = body_shell
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("metrics body layout areas");
    assert_eq!(areas[0], ["m0", "m1", "m2"]);
    assert_eq!(
        body_shell
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_2_1")
    );
    let metric_tiles: Vec<_> = body_shell
        .blocks
        .iter()
        .filter(|node| {
            matches!(
                node,
                crate::UiNodeDecl::Panel(panel) if panel.id.starts_with("metric_")
            )
        })
        .collect();
    assert_eq!(metric_tiles.len(), 3);
    let wide = body_shell
        .blocks
        .iter()
        .find_map(|node| match node {
            crate::UiNodeDecl::Panel(panel) if panel.id == "metric_m2" => Some(panel),
            _ => None,
        })
        .expect("metric_m2");
    let wide_areas = wide
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("wide metric layout");
    assert_eq!(wide_areas[0], ["top", "top", "top"]);
    assert_eq!(wide_areas[1], ["b0", "b1", "b2"]);
    assert_eq!(
        wide.props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metric_compound_2_1")
    );
    let layout_areas = metrics
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("title+metrics shell layout");
    assert_eq!(layout_areas[0], ["head"]);
    assert_eq!(layout_areas[1], ["body"]);
}

#[test]
fn compile_cockpit_metric_gallery_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("05-panel");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: Some("metric.mei".to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile 05-panel/metric.mei failed: {error}"));
    assert_eq!(compiled.active_target_file, "metric.mei");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "metric.mei should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert_eq!(sc.scene.id, "home");
    assert!(
        sc.scene
            .summary
            .as_deref()
            .is_some_and(|value| value.contains("stack_desc") || value.contains("metric_card")),
        "metric.mei scene summary should describe metric_card gallery"
    );
    for panel_id in ["demo_row", "demo_column", "demo_stack", "demo_stack_desc"] {
        assert!(
            sc.panels.iter().any(|p| p.id == panel_id),
            "panel {panel_id} missing; got {:?}",
            sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_cockpit_metric_data_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("05-panel");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: Some("metric-data.mei".to_string()),
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
        sc.scene
            .summary
            .as_deref()
            .is_some_and(|value| value.contains("metric_ref") || value.contains("static object")),
        "metric-data.mei summary should describe binding demo"
    );
    fn collect_panel_ids(panels: &[crate::PanelDecl], out: &mut Vec<String>) {
        for panel in panels {
            out.push(panel.id.clone());
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
                    collect_panel_ids(&[nested.clone()], out);
                }
            }
        }
    }
    let mut panel_ids = Vec::new();
    collect_panel_ids(&sc.panels, &mut panel_ids);
    assert!(
        panel_ids.iter().any(|id| id == "binding_demo"),
        "binding_demo panel missing; got {:?}",
        panel_ids
    );
    assert!(
        panel_ids.iter().any(|id| id == "static_demo"),
        "static_demo missing; got {:?}",
        panel_ids
    );
    fn collect_use_keys(nodes: &[crate::UiNodeDecl], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                crate::UiNodeDecl::Block(block) => out.push(block.use_key.clone()),
                crate::UiNodeDecl::Panel(panel) => collect_use_keys(&panel.blocks, out),
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
        tile_count, 2,
        "metric-data.mei should use two metric_ref-driven tiles; got keys: {:?}",
        use_keys
    );
}

#[test]
fn compile_cockpit_qunfu_chrome_includes_body_shell_panel() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/cockpit");
    let app_root = source_root.join("03-spbjw-qunfu-chrome");
    let compiled = compile_app_from_root(&source_root, &app_root).unwrap();
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert!(
        sc.panels.iter().any(|p| p.id == "body_shell"),
        "body_shell panel must compile; got ids: {:?}",
        sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
}
