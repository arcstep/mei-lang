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
        "10-metric-card-template-clone",
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
fn compile_ds_04_data_table_features_example() {
    let root = workspace_root();
    let source_root = root.join("workspaces/examples/ds");
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
fn compile_cockpit_panel_screen_header_preview() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("templates/cockpit");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("panel/panel-screen-header.mei".to_string()),
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

#[test]
fn compile_cockpit_templates_preview() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("templates/cockpit");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("metric".to_string()),
            preview_target: None,
        },
    )
    .unwrap_or_else(|error| panic!("compile templates/cockpit failed: {error}"));
    assert_eq!(compiled.active_target_file, "metric-card.mei");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "templates/cockpit metric scene should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "templates/cockpit metric scene should produce a scene contract"
    );
    let sc = compiled.scene_contract.as_ref().expect("scene contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
                    if let Some(found) = find_panel_by_id(std::slice::from_ref(nested), target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    let solid_row = find_panel_by_id(&sc.panels, "solid_row_body").expect("solid_row_body");
    assert_eq!(
        solid_row
            .props
            .get("__mei_layout_policy")
            .and_then(|v| v.as_str()),
        Some("metrics_strip")
    );
    let layout = solid_row.layout.as_ref().expect("solid_row_body layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![vec!["m0".to_string(), "m1".to_string()]])
    );
    let accent = find_panel_by_id(&sc.panels, "preview_row_accent").expect("preview_row_accent");
    let accent_layout = accent.layout.as_ref().expect("accent layout");
    assert_eq!(
        accent_layout.areas.as_ref(),
        Some(&vec![vec![
            "label".to_string(),
            "value".to_string(),
            "unit".to_string()
        ]])
    );
    assert_eq!(
        accent_layout.justify.as_deref(),
        Some("start"),
        "row solid cards should use justify=start for inner label/value/unit grid"
    );
    let stack_desc =
        find_panel_by_id(&sc.panels, "preview_stack_desc").expect("preview_stack_desc");
    let stack_desc_layout = stack_desc.layout.as_ref().expect("stack_desc layout");
    assert_eq!(
        stack_desc_layout.rows.as_deref(),
        Some(
            &[
                "14px".to_string(),
                "auto".to_string(),
                "54px".to_string(),
                "6px".to_string(),
                "20px".to_string(),
                "14px".to_string(),
            ][..]
        ),
        "stack_desc mid card must keep 14px top band and 54px value band (value bottom 40px from card)"
    );
    assert!(
        find_panel_by_id(&sc.panels, "preview_progress").is_some(),
        "templates preview should include progress metric card section"
    );
    let long = find_panel_by_id(&sc.panels, "preview_long").expect("preview_long");
    let long_layout = long.layout.as_ref().expect("preview_long layout");
    assert_eq!(
        long_layout.areas.as_ref(),
        Some(&vec![
            vec!["main".to_string(), "rtop".to_string()],
            vec!["main".to_string(), "rbottom".to_string()],
        ]),
        "long compound must use left stack + right top/bottom rows"
    );
    assert_eq!(
        long_layout.columns.as_deref(),
        Some(&["1fr".to_string(), "2fr".to_string()][..]),
    );
    assert!(
        long.props
            .get("background")
            .and_then(|v| v.get("image"))
            .and_then(|v| v.as_str())
            .is_some_and(|url| url.contains("metric-bg-long")),
        "long compound shell should use metric-bg-long asset"
    );
    let progress_patch =
        find_panel_by_id(&sc.panels, "preview_progress_patch").expect("preview_progress_patch");
    let has_progress_block = progress_patch.blocks.iter().any(|node| {
        let crate::UiNodeDecl::Block(block) = node else {
            return false;
        };
        block.use_key == "cockpit.metric-progress"
            && block
                .props
                .get("value")
                .and_then(|v| v.as_str())
                .is_some_and(|value| value == "65" || value == "65%")
    });
    assert!(
        has_progress_block,
        "cloned progress template + patch.desc should lower desc slot to metric-progress"
    );
    let patch_layout = progress_patch
        .layout
        .as_ref()
        .expect("progress_patch layout");
    assert!(
        patch_layout
            .areas
            .as_ref()
            .is_some_and(|rows| rows.iter().flatten().any(|cell| cell == "desc")),
        "cloned progress template + source must keep template stack_desc grid with desc area"
    );
    fn block_v_align(panel: &crate::PanelDecl, role: &str) -> Option<String> {
        for node in &panel.blocks {
            let crate::UiNodeDecl::Block(block) = node else {
                continue;
            };
            if block.props.get("metric_role").and_then(|v| v.as_str()) != Some(role) {
                continue;
            }
            return block
                .props
                .get("metric_v_align")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        None
    }
    assert_eq!(
        block_v_align(&progress_patch, "label").as_deref(),
        Some("center")
    );
    assert_eq!(
        block_v_align(&progress_patch, "value").as_deref(),
        Some("end")
    );
    assert_eq!(
        block_v_align(&progress_patch, "unit").as_deref(),
        Some("end")
    );
    let compound_top =
        find_panel_by_id(&sc.panels, "compound_top").expect("compound_top metric card");
    assert_eq!(
        block_v_align(compound_top, "label").as_deref(),
        Some("center"),
        "label_vertical_align on metric_card must reach mei-text, not card_plain defaults"
    );
    assert_eq!(
        block_v_align(compound_top, "value").as_deref(),
        Some("center")
    );
    let long_main = find_panel_by_id(&sc.panels, "long_main").expect("long_main metric card");
    assert_eq!(block_v_align(long_main, "label").as_deref(), Some("end"));
    assert_eq!(block_v_align(long_main, "value").as_deref(), Some("center"));
    assert_eq!(block_v_align(long_main, "unit").as_deref(), Some("center"));
    assert_eq!(
        progress_patch
            .props
            .get("__mei_metric_template")
            .and_then(|v| v.as_str()),
        Some("stack_desc"),
        "base clone + source must not overwrite template stack_desc with default stack"
    );
    let patch_rows = patch_layout.rows.as_ref().expect("progress_patch rows");
    assert!(
        patch_rows.iter().any(|track| track.contains("18px")),
        "progress_patch must keep template fixed label row track, not 1fr 1fr stack bands: {:?}",
        patch_rows
    );
    assert!(
        !patch_rows.iter().any(|track| track == "1fr") || patch_rows.len() > 2,
        "progress_patch should not collapse to two-band 1fr stack rows: {:?}",
        patch_rows
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
            diag.code != "layout_eval_row_budget_overflow"
                && diag.code != "layout_eval_column_budget_overflow"
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
        compiled.diagnostics.iter().all(|diag| {
            !(diag.code.starts_with("layout_eval_")
                && matches!(diag.severity, crate::Severity::Error))
        }),
        "05-panel should not produce blocking layout eval diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        sc.panels.iter().any(|p| p.id == "block_title_metrics_bg"),
        "panel block_title_metrics_bg must compile; got ids: {:?}",
        sc.panels.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    assert!(sc.panels.iter().any(|p| p.id == "block_title_metrics_grid"));
    assert!(sc
        .panels
        .iter()
        .any(|p| p.id == "block_title_metrics_focus"));
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
        Some("metrics_auto")
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
    let grid_shell = sc
        .panels
        .iter()
        .find(|p| p.id == "block_title_metrics_grid")
        .and_then(|panel| {
            panel.blocks.iter().find_map(|node| match node {
                crate::UiNodeDecl::Panel(nested) if nested.id == "metrics_grid_body" => {
                    Some(nested)
                }
                _ => None,
            })
        })
        .expect("metrics_grid_body");
    let grid_areas = grid_shell
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("metrics grid layout");
    assert_eq!(grid_areas[0], ["m0", "m1", "m2"]);
    assert_eq!(grid_areas[1], ["m3", "m4", "m5"]);
    let focus_shell = sc
        .panels
        .iter()
        .find(|p| p.id == "block_title_metrics_focus")
        .and_then(|panel| {
            panel.blocks.iter().find_map(|node| match node {
                crate::UiNodeDecl::Panel(nested) if nested.id == "metrics_focus_body" => {
                    Some(nested)
                }
                _ => None,
            })
        })
        .expect("metrics_focus_body");
    let focus_areas = focus_shell
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("metrics focus layout");
    assert_eq!(focus_areas[0], ["m0", "m1", "m2", "m3"]);
    assert_eq!(focus_areas[1], ["m4", "m4", "m4", "m4"]);
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
                crate::UiNodeDecl::Panel(nested) if nested.id == "binding_demo_grid" => {
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
                crate::UiNodeDecl::Panel(nested) if nested.id == "binding_demo_wide" => {
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
        tile_count, 0,
        "metric-data.mei should not rely on qunfu-metric-tile; got keys: {:?}",
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
