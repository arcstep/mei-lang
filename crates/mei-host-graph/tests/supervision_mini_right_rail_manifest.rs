use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_supervision_mini_imported() {
    INIT.call_once(|| {
        let workspace = ws_demo_v2();
        let bundle = workspace
            .join("apps/supervision-mini/env/current/build/exchange/supervision-mini.meibundle");
        assert!(
            bundle.is_file(),
            "run `mei-compiler compile --workspace ws-demo-v2 --app supervision-mini` first"
        );
        let ctx = HostContext::new(workspace, "supervision-mini");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import supervision-mini bundle");
    });
}

fn supervision_mini_home_outcome() -> mei_host_graph::AssembleOutcome {
    ensure_supervision_mini_imported();
    clear_assemble_cache_for_app("supervision-mini");
    assemble_scope_from_registry(ws_demo_v2().as_path(), "supervision-mini", "home")
        .expect("assemble")
        .expect("home outcome")
}

fn find_panel<'a>(
    panels: &'a [mei_lang_kernel::UiNodeDecl],
    target: &str,
) -> Option<&'a mei_lang_kernel::UiNodeDecl> {
    for panel in panels {
        if panel.id == target {
            return Some(panel);
        }
        if let Some(found) = panel.blocks.iter().find_map(|node| match node {
            mei_lang_kernel::UiTreeNode::Panel(child) => {
                find_panel(std::slice::from_ref(child), target)
            }
            _ => None,
        }) {
            return Some(found);
        }
    }
    None
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
fn supervision_mini_right_rail_region_manifest_includes_four_row_grid() {
    let outcome = supervision_mini_home_outcome();
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
                "1.15fr".to_string(),
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
        Some("1fr 1.15fr 1fr 1fr")
    );
    assert_eq!(
        region_entry.grid_template_areas.as_deref(),
        Some("'warning' 'enforcement' '_' '_'")
    );
}

#[test]
fn supervision_mini_warning_head_exports_head_chrome() {
    let outcome = supervision_mini_home_outcome();
    let head_doc = supervision_mini_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| {
            doc.slot_group_id == "scope:t1/right_rail/warning/title_zone"
                || doc.slot_group_id == "scope:t1/right_rail/warning/head"
        })
        .expect("scope:t1/right_rail/warning/title_zone eval doc");
    let head_slot = head_doc
        .slots
        .get("t1/right_rail/warning/title_zone")
        .or_else(|| head_doc.slots.get("t1/right_rail/warning/head"))
        .expect("warning title_zone/head slot");
    let head_chrome = head_slot
        .get("head_chrome")
        .expect("head_chrome on warning head slot");
    assert_eq!(head_chrome["title"], "监督预警");
    assert_eq!(head_chrome["caret"]["enabled"], true);
    let cell_style = head_chrome["cell_style"].as_str().unwrap_or("");
    assert!(
        cell_style.contains("linear-gradient")
            || cell_style.contains("panel_title_bar")
            || cell_style.contains("url("),
        "expected resolved panel_title_bar gradient/image in cell_style: {cell_style}"
    );
}

#[test]
fn supervision_mini_metric_card_value_mount_includes_resolved_popup() {
    let outcome = supervision_mini_home_outcome();
    let docs = supervision_mini_home_eval_docs(&outcome);
    let mut value_popup = None;
    for doc in &docs {
        for (scope, slot) in &doc.slots {
            if !scope.contains("supervision_triptych_first") {
                continue;
            }
            let mounts = slot
                .get("component_mounts")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
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
    let popup = value_popup.expect("supervision_triptych_first value popup in component_mounts");
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
fn supervision_mini_enforcement_objects_panel_has_slot_frame_background() {
    let outcome = supervision_mini_home_outcome();
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let objects = find_panel(panels, "enforcement_objects").expect("enforcement_objects panel");
    let bg = objects
        .props
        .get("background")
        .expect("background on shell");
    let bg_json = serde_json::to_string(bg).unwrap_or_default();
    assert!(
        bg_json.contains("metric-bg-target"),
        "expected metric-bg-target on enforcement_objects shell, got {bg_json}"
    );
    assert_eq!(
        objects
            .props
            .get("__mei_slot_frame_bg")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    let body = find_panel(panels, "enforcement_objects_body").expect("enforcement_objects_body");
    assert!(
        body.layout
            .as_ref()
            .and_then(|layout| layout.areas.as_ref())
            .is_some_and(|rows| rows.iter().flatten().any(|area| area == "top")),
        "compound grid body should keep named slot areas"
    );
}

#[test]
fn supervision_mini_enforcement_compound_metric_exports_panel_shell_background() {
    let outcome = supervision_mini_home_outcome();
    let compound_doc = supervision_mini_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| {
            doc.slots.values().any(|slot| {
                slot.get("content_kind").and_then(|value| value.as_str()) == Some("compound-metric")
            })
        })
        .expect("compound-metric eval slot group");
    let compound_slot = compound_doc
        .slots
        .values()
        .find(|slot| {
            slot.get("content_kind").and_then(|value| value.as_str()) == Some("compound-metric")
        })
        .expect("compound-metric slot");
    let panel_shell = compound_slot
        .get("panel_shell")
        .expect("panel_shell on compound-metric slot");
    let bg = serde_json::to_string(&panel_shell["props"]["background"]).unwrap_or_default();
    assert!(
        bg.contains("metric-bg-target"),
        "compound-metric slot should export metric-bg-target panel_shell, got {bg}"
    );
    assert_eq!(
        panel_shell["props"]["__mei_slot_frame_bg"].as_bool(),
        Some(true)
    );
}

#[test]
fn supervision_mini_enforcement_compound_metric_exports_static_mounts() {
    let outcome = supervision_mini_home_outcome();
    let docs = supervision_mini_home_eval_docs(&outcome);
    let enforcement_groups: Vec<_> = docs
        .iter()
        .filter(|doc| doc.slot_group_id.contains("enforcement"))
        .map(|doc| doc.slot_group_id.clone())
        .collect();
    let slot = docs
        .into_iter()
        .find(|doc| doc.slot_group_id.contains("/top") && doc.slot_group_id.contains("enforcement"))
        .and_then(|doc| {
            doc.slots
                .iter()
                .find(|(scope, _)| scope.contains("/top"))
                .map(|(_, slot)| slot.clone())
        })
        .unwrap_or_else(|| panic!("enforcement top eval slot; groups={enforcement_groups:#?}"));
    let mounts = slot
        .get("component_mounts")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        mounts.len() >= 4,
        "expected metric shell + label/value/unit mounts, got {mounts:#?}"
    );
    let value_mount = mounts.iter().find(|mount| {
        mount
            .get("props")
            .and_then(|props| props.get("metric_role"))
            .and_then(|role| role.as_str())
            == Some("value")
    });
    let value_content = value_mount
        .and_then(|mount| mount.get("props"))
        .and_then(|props| props.get("content"))
        .and_then(|content| content.get("value"))
        .and_then(|value| value.as_str());
    assert_eq!(
        value_content,
        Some("16.4"),
        "static source value should export in component_mounts"
    );
}

#[test]
fn supervision_mini_supervision_stats_exports_panel_shell() {
    let outcome = supervision_mini_home_outcome();
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
fn supervision_mini_triptych_metric_exports_slot_frame_panel_shell() {
    let outcome = supervision_mini_home_outcome();
    let docs = supervision_mini_home_eval_docs(&outcome);
    let first_shell = docs.iter().find_map(|doc| {
        doc.slots.iter().find_map(|(scope, slot)| {
            if !scope.contains("supervision_triptych_first") {
                return None;
            }
            slot.get("panel_shell")
                .cloned()
                .map(|shell| (scope.clone(), shell))
        })
    });
    let (scope, shell) = first_shell.expect("panel_shell on supervision_triptych_first*");
    assert!(
        scope.contains("supervision_triptych_first"),
        "unexpected scope {scope}"
    );
    assert_eq!(
        shell["props"]["__mei_slot_frame_bg"].as_bool(),
        Some(true),
        "triptych metric shell must keep slot-frame flag: {shell}"
    );
    let bg = serde_json::to_string(&shell["props"]["background"]).unwrap_or_default();
    assert!(
        bg.contains("#71F1EA") || bg.contains("71F1EA") || bg.contains("rgba(98,190,235"),
        "triptych metric shell must export corner decor background, got {bg}"
    );
}

#[test]
fn supervision_mini_screen_header_exports_bare_panel_shell() {
    let outcome = supervision_mini_home_outcome();
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
