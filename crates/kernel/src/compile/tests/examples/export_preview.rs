use serde_json::Value;

use super::super::super::{compile_app_from_root, compile_app_from_root_with_options, CompileOptions};
use super::super::harness::{dev_examples_root, dev_workspace_root};

#[test]
fn compile_cockpit_templates_preview() {
    let source_root = dev_workspace_root();
    let app_root = source_root.join(".stock/templates/cockpit");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("metric".to_string()),
            preview_target: None,
            ..Default::default()
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
        panels: &'a [crate::UiNodeDecl],
        target: &str,
    ) -> Option<&'a crate::UiNodeDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiTreeNode::Panel(nested) = node {
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
        let crate::UiTreeNode::Block(block) = node else {
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
    fn block_v_align(panel: &crate::UiNodeDecl, role: &str) -> Option<String> {
        for node in &panel.blocks {
            let crate::UiTreeNode::Block(block) = node else {
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
    let source_root = dev_examples_root().join("cockpit");
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
            crate::UiTreeNode::Panel(panel) if panel.id == "metrics_body" => Some(panel),
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
                crate::UiTreeNode::Panel(panel) if panel.id.starts_with("metric_")
            )
        })
        .collect();
    assert_eq!(metric_tiles.len(), 3);
    let wide = body_shell
        .blocks
        .iter()
        .find_map(|node| match node {
            crate::UiTreeNode::Panel(panel) if panel.id == "metric_m2" => Some(panel),
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
                crate::UiTreeNode::Panel(nested) if nested.id == "metrics_grid_body" => {
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
                crate::UiTreeNode::Panel(nested) if nested.id == "metrics_focus_body" => {
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
    let source_root = dev_examples_root().join("cockpit");
    let app_root = source_root.join("05-panel");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: Some("metric.mei".to_string()),
            ..Default::default()
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

