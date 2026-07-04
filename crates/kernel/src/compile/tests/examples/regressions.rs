use std::fs;

use serde_json::Value;

use super::super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::super::harness::{build_regression_workspace_root, dev_examples_root, dev_workspace_root};
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
    let source_root = dev_examples_root().join("core");
    for app_id in [
        "01-single-file-doc",
        "02-external-scene-file",
        "03-multi-panel-baseline",
        "08-scene-export-resource",
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
    let source_root = dev_examples_root().join("sim");
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
fn compile_core_invalid_examples_report_expected_errors() {
    let source_root = dev_examples_root().join("core/_invalid");
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
    let source_root = dev_examples_root().join("refs");
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
        "10-metric-card-template-clone",
        "11-exported-templates",
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
fn compile_scene_export_preview_enriches_file_tree_children() {
    let source_root = dev_examples_root().join("core");
    let app_root = source_root.join("08-scene-export-resource");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("detail".to_string()),
            preview_target: Some("exports.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile scene export preview failed: {error}"));
    let mut nodes = Vec::new();
    walk_file_tree(&compiled.file_tree, &mut nodes);
    let exports_node = nodes
        .into_iter()
        .find(|node| node.path == "exports.mei" && node.kind == "file")
        .unwrap_or_else(|| {
            panic!(
                "exports.mei missing from file_tree: {:?}",
                compiled.file_tree
            )
        });
    assert_eq!(exports_node.children.len(), 2);
    let overview = exports_node
        .children
        .iter()
        .find(|child| {
            child.kind == "scene_export" && child.scene_export_id.as_deref() == Some("overview")
        })
        .unwrap_or_else(|| panic!("overview scene_export missing"));
    assert_eq!(overview.name, "scene_export 概览场景");
    assert_eq!(overview.semantic_label.as_deref(), Some("overview"));
    assert_eq!(overview.mei_kind.as_deref(), Some("scene"));
    assert!(exports_node
        .children
        .iter()
        .any(|child| child.kind == "scene_export"
            && child.scene_export_id.as_deref() == Some("detail")));
}

fn walk_file_tree<'a>(nodes: &'a [crate::WorkspaceNode], out: &mut Vec<&'a crate::WorkspaceNode>) {
    for node in nodes {
        out.push(node);
        walk_file_tree(&node.children, out);
    }
}

#[test]
fn compile_scene_export_preview_target_selects_requested_export() {
    let source_root = dev_examples_root().join("core");
    let app_root = source_root.join("08-scene-export-resource");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("detail".to_string()),
            preview_target: Some("exports.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile scene export preview failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scene export preview should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("detail"));
    assert_eq!(
        compiled
            .scene_contract
            .as_ref()
            .map(|contract| contract.scene.id.as_str()),
        Some("detail")
    );
}

#[test]
fn compile_refs_invalid_examples_report_expected_errors() {
    let source_root = dev_examples_root().join("refs/_invalid");
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
fn compile_capability_examples_baselines() {
    let source_root = dev_examples_root().join("capability");
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
fn compile_ds_04_data_table_features_example() {
    let source_root = dev_examples_root().join("ds");
    let app_root = source_root.join("04-data-table-features");
    let compiled = compile_app_from_root(&source_root, &app_root)
        .unwrap_or_else(|error| panic!("compile ds-04-data-table-features failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "ds-04-data-table-features should compile without errors: {:?}",
        compiled.diagnostics
    );
    for scene_id in [
        "index",
        "layout_default_flow",
        "manage_server_paging",
        "manage_query_state",
        "cockpit_embedded_carousel",
        "cockpit_metric_runtime",
        "cockpit_warnings_skin",
        "layout_stage_contain",
        "metric_explain_access",
    ] {
        assert!(
            compiled
                .scene_routes
                .iter()
                .any(|route| route.scene_id == scene_id),
            "missing scene route for {scene_id}"
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
    let source_root = dev_examples_root().join("cockpit");
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
    let source_root = dev_examples_root().join("cockpit");
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
fn compile_cockpit_panel_screen_header_preview() {
    let source_root = dev_workspace_root();
    let app_root = source_root.join(".stock/templates/cockpit");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("panel/panel-screen-header.mei".to_string()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("compile panel-screen-header failed: {error}"));
    assert_eq!(compiled.active_target_file, "panel/panel-screen-header.mei");
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    assert_eq!(sc.panels.len(), 1, "single header shell panel");
    assert_eq!(sc.panels[0].id, "screen_header_shell");
    let frame = sc.frame.as_ref().expect("frame");
    let frame_props = frame.props.as_object().expect("frame props");
    assert!(
        frame_props.get("viewport").is_some(),
        "panel-screen-header should declare 1920 viewport for manage preview; layout={:?} props={:?}",
        frame.layout,
        frame_props
    );
}

