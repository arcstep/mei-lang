use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn find_panel<'a>(
    panels: &'a [mei_lang_kernel::PanelDecl],
    target: &str,
) -> Option<&'a mei_lang_kernel::PanelDecl> {
    for panel in panels {
        if panel.id == target {
            return Some(panel);
        }
        if let Some(found) = panel
            .blocks
            .iter()
            .find_map(|node| match node {
                mei_lang_kernel::UiNodeDecl::Panel(child) => find_panel(std::slice::from_ref(child), target),
                _ => None,
            })
        {
            return Some(found);
        }
    }
    None
}

#[test]
fn supervision_mini_right_rail_region_manifest_includes_four_row_grid() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let right_rail = find_panel(panels, "right_rail").expect("right_rail panel");
    let layout = right_rail.layout.as_ref().expect("right_rail layout");
    assert_eq!(
        layout.rows.as_deref(),
        Some(
            &[
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string()
            ][..]
        ),
        "author layout rows on right_rail panel"
    );

    let manifest = outcome
        .compiled
        .ui_layout_index
        .layout_budget_manifest("test");
    let region_entry = manifest
        .entries
        .get("t1/right_rail")
        .expect("t1/right_rail region manifest entry");
    assert_eq!(
        region_entry.grid_template_rows.as_deref(),
        Some("1fr 1fr 1fr 1fr")
    );
    assert_eq!(
        region_entry.grid_template_areas.as_deref(),
        Some("'warning' '_' '_' '_'")
    );
}

fn supervision_mini_home_eval_docs(
    outcome: &mei_host_graph::AssembleOutcome,
) -> Vec<mei_host_graph::EvalSlotGroupDocument> {
    use mei_host_graph::{build_eval_slot_group_document, collect_slot_groups};
    use mei_lang_kernel::DataMode;

    let structure = mei_host_graph::build_structure_full_document(&outcome.compiled, "test");
    collect_slot_groups(&structure)
        .into_iter()
        .map(|group| {
            build_eval_slot_group_document(
                &outcome.compiled,
                &structure,
                group.as_str(),
                DataMode::Eval,
                Some(ws_demo_v2().as_path()),
            )
        })
        .collect()
}

#[test]
fn supervision_mini_warning_head_exports_head_chrome() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome");
    let head_doc = supervision_mini_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| doc.slot_group_id == "scope:t1/right_rail/warning/head")
        .expect("scope:t1/right_rail/warning/head eval doc");
    let head_slot = head_doc
        .slots
        .get("t1/right_rail/warning/head")
        .expect("warning head slot");
    let head_chrome = head_slot
        .get("head_chrome")
        .expect("head_chrome on warning head slot");
    assert_eq!(head_chrome["title"], "监督预警");
    assert_eq!(head_chrome["caret"]["enabled"], true);
    let cell_style = head_chrome["cell_style"].as_str().unwrap_or("");
    assert!(
        cell_style.contains("linear-gradient"),
        "expected resolved panel_title_bar gradient in cell_style: {cell_style}"
    );
}

#[test]
fn supervision_mini_metric_card_value_mount_includes_resolved_popup() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome");
    let docs = supervision_mini_home_eval_docs(&outcome);
    let mut value_popup = None;
    for doc in &docs {
        for (scope, slot) in &doc.slots {
            if !scope.contains("supervision_items_card") {
                continue;
            }
            let mounts = slot
                .get("component_mounts")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            eprintln!("scope={scope} mounts={}", mounts.len());
            for mount in &mounts {
                let props = mount.get("props").and_then(|value| value.as_object());
                let role = props
                    .and_then(|map| map.get("metric_role"))
                    .and_then(|value| value.as_str());
                if role == Some("value") {
                    value_popup = props.and_then(|map| map.get("popup")).cloned();
                }
            }
        }
    }
    let popup = value_popup.expect("supervision_items_card value popup in component_mounts");
    assert!(
        popup.get("__ref").and_then(|value| value.as_str()) != Some("link_ref"),
        "popup should be resolved: {popup}"
    );
    assert!(
        popup.get("mode").and_then(|value| value.as_str()) == Some("popup")
            || popup.get("mode").and_then(|value| value.as_str()) == Some("board_link"),
        "unexpected popup mode: {popup}"
    );
}

#[test]
fn supervision_mini_supervision_stats_exports_panel_shell() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome");
    let stats_doc = supervision_mini_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| doc.slot_group_id == "scope:t1/right_rail/warning/supervision-stats")
        .expect("scope:t1/right_rail/warning/supervision-stats eval doc");
    let stats_slot = stats_doc
        .slots
        .get("t1/right_rail/warning/supervision-stats")
        .expect("supervision-stats slot");
    let panel_shell = stats_slot
        .get("panel_shell")
        .expect("panel_shell on supervision-stats");
    assert_eq!(panel_shell["mount_role"], "panel-shell");
    let bg = panel_shell["props"]["background"].as_str().unwrap_or("");
    assert!(
        bg.contains("linear-gradient") || bg.contains("rgba"),
        "expected resolved panel_glow_bg: {bg}"
    );
}

#[test]
fn supervision_mini_screen_header_exports_bare_panel_shell() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome");
    let docs = supervision_mini_home_eval_docs(&outcome);
    let header_shell = docs.iter().find_map(|doc| {
        doc.slots.iter().find_map(|(scope, slot)| {
            if !scope.contains("header") {
                return None;
            }
            let shell = slot.get("panel_shell")?;
            if shell["props"]["chrome"].as_str() == Some("bare") {
                Some((scope.clone(), shell.clone()))
            } else {
                None
            }
        })
    });
    let (scope, shell) = header_shell.expect("bare panel_shell on header slot");
    assert!(
        scope.contains("header"),
        "expected header scope, got {scope}"
    );
    assert_eq!(shell["props"]["padding"], "0");
    assert_eq!(shell["props"]["border"], "none");
}
