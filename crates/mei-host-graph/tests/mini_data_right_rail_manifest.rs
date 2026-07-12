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

fn ensure_mini_data_imported() {
    INIT.call_once(|| {
        let workspace = ws_demo_v2();
        let bundle =
            workspace.join("apps/mini-data/env/current/build/exchange/mini-data.meibundle");
        assert!(
            bundle.is_file(),
            "run `mei-compiler compile --workspace ws-demo-v2 --app mini-data` first"
        );
        let ctx = HostContext::new(workspace, "mini-data");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import mini-data bundle");
    });
}

fn mini_data_home_outcome() -> mei_host_graph::AssembleOutcome {
    ensure_mini_data_imported();
    clear_assemble_cache_for_app("mini-data");
    assemble_scope_from_registry(ws_demo_v2().as_path(), "mini-data", "home")
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

fn mini_data_home_eval_docs(
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
fn mini_data_right_rail_region_manifest_includes_four_row_grid() {
    let outcome = mini_data_home_outcome();
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
        Some("'warning' 'enforcement' '.' '.'")
    );
}

#[test]
fn mini_data_warning_head_exports_head_chrome() {
    let outcome = mini_data_home_outcome();
    let head_doc = mini_data_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| {
            doc.slot_group_id == "scope:t1/right_rail/warning/title"
                || doc.slot_group_id == "scope:t1/right_rail/warning/title_zone"
                || doc.slot_group_id == "scope:t1/right_rail/warning/head"
        })
        .expect("scope:t1/right_rail/warning/title eval doc");
    let head_slot = head_doc
        .slots
        .get("t1/right_rail/warning/title")
        .or_else(|| head_doc.slots.get("t1/right_rail/warning/title_zone"))
        .or_else(|| head_doc.slots.get("t1/right_rail/warning/head"))
        .expect("warning title/title_zone/head slot");
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
fn mini_data_metric_card_value_mount_includes_resolved_popup() {
    let outcome = mini_data_home_outcome();
    let docs = mini_data_home_eval_docs(&outcome);
    let mut value_popup = None;
    for doc in &docs {
        for (scope, slot) in &doc.slots {
            if !(scope.contains("/items") || scope.contains("supervision_triptych_first")) {
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
    let popup = value_popup.expect("items/triptych value popup in component_mounts");
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
fn mini_data_enforcement_objects_panel_has_slot_frame_background() {
    let outcome = mini_data_home_outcome();
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let objects = find_panel(panels, "objects")
        .or_else(|| find_panel(panels, "enforcement_objects"))
        .expect("objects/enforcement_objects panel");
    let bg = objects
        .props
        .get("background")
        .expect("background on shell");
    let bg_json = serde_json::to_string(bg).unwrap_or_default();
    assert!(
        bg_json.contains("metric-bg-target"),
        "expected metric-bg-target on objects shell, got {bg_json}"
    );
    assert_eq!(
        objects
            .props
            .get("__mei_slot_frame_bg")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(
        objects
            .layout
            .as_ref()
            .and_then(|layout| layout.areas.as_ref())
            .is_some_and(|rows| {
                rows.iter()
                    .flatten()
                    .any(|area| area == "total" || area == "enterprise" || area == "top")
            }),
        "compound panel should keep named slot areas"
    );
}

#[test]
fn mini_data_enforcement_compound_metric_exports_panel_shell_background() {
    let outcome = mini_data_home_outcome();
    let compound_doc = mini_data_home_eval_docs(&outcome)
        .into_iter()
        .find(|doc| {
            doc.slot_group_id.contains("/enforcement/objects")
                && doc
                    .slots
                    .values()
                    .any(|slot| slot.get("panel_shell").is_some())
        })
        .expect("enforcement/objects eval slot group with panel_shell");
    let compound_slot = compound_doc
        .slots
        .values()
        .find(|slot| slot.get("panel_shell").is_some())
        .expect("objects panel_shell slot");
    let panel_shell = compound_slot
        .get("panel_shell")
        .expect("panel_shell on objects slot");
    let bg = serde_json::to_string(&panel_shell["props"]["background"]).unwrap_or_default();
    assert!(
        bg.contains("metric-bg-target"),
        "objects slot should export metric-bg-target panel_shell, got {bg}"
    );
    assert_eq!(
        panel_shell["props"]["__mei_slot_frame_bg"].as_bool(),
        Some(true)
    );
}

#[test]
fn mini_data_enforcement_compound_metric_exports_static_mounts() {
    let outcome = mini_data_home_outcome();
    let docs = mini_data_home_eval_docs(&outcome);
    let enforcement_groups: Vec<_> = docs
        .iter()
        .filter(|doc| doc.slot_group_id.contains("enforcement"))
        .map(|doc| doc.slot_group_id.clone())
        .collect();
    let slot = docs
        .into_iter()
        .find(|doc| {
            doc.slot_group_id.contains("enforcement")
                && (doc.slot_group_id.contains("/total") || doc.slot_group_id.contains("/top"))
        })
        .and_then(|doc| {
            doc.slots
                .iter()
                .find(|(scope, _)| scope.contains("/total") || scope.contains("/top"))
                .map(|(_, slot)| slot.clone())
        })
        .unwrap_or_else(|| {
            panic!("enforcement total/top eval slot; groups={enforcement_groups:#?}")
        });
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
fn mini_data_supervision_stats_exports_panel_shell() {
    let outcome = mini_data_home_outcome();
    let stats_doc = mini_data_home_eval_docs(&outcome)
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
fn mini_data_triptych_metric_exports_slot_frame_panel_shell() {
    let outcome = mini_data_home_outcome();
    let docs = mini_data_home_eval_docs(&outcome);
    let items_slot = docs
        .iter()
        .find(|doc| doc.slot_group_id.contains("warning") && doc.slot_group_id.contains("/items"))
        .and_then(|doc| {
            doc.slots.values().find(|slot| {
                slot.get("component_mounts")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| !a.is_empty())
            })
        })
        .expect("warning items metric content slot");
    let mounts = items_slot
        .get("component_mounts")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        mounts.len() >= 4,
        "expected metric shell + label/value/unit mounts, got {mounts:#?}"
    );
    let shell_mount = mounts.iter().find(|mount| {
        mount
            .get("props")
            .and_then(|props| props.get("__mei_slot_frame_bg"))
            .and_then(|flag| flag.as_bool())
            == Some(true)
            || mount.get("mount_role").and_then(|v| v.as_str()) == Some("panel-shell")
            || mount
                .get("props")
                .and_then(|props| props.get("background"))
                .is_some()
    });
    let shell = shell_mount.expect("slot-frame / background mount on items metric");
    let bg = serde_json::to_string(shell.get("props").unwrap_or(shell)).unwrap_or_default();
    assert!(
        bg.contains("#71F1EA")
            || bg.contains("71F1EA")
            || bg.contains("rgba(98,190,235")
            || bg.contains("metric-bg")
            || bg.contains("linear-gradient"),
        "narrow metric shell must export corner decor background, got {bg}"
    );
}

#[test]
fn mini_data_screen_header_exports_bare_panel_shell() {
    let outcome = mini_data_home_outcome();
    let docs = mini_data_home_eval_docs(&outcome);
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

#[test]
fn mini_data_hierarchy_spacing_omitted_defaults_are_injected() {
    let outcome = mini_data_home_outcome();
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;

    let t1 = find_panel(panels, "t1").expect("t1 plane panel");
    assert_eq!(
        t1.layout.as_ref().and_then(|l| l.gap.as_deref()),
        Some("1px"),
        "plane omit → region outer margin 1px"
    );
    assert_eq!(
        t1.body_props
            .get("padding")
            .and_then(|v| v.as_str())
            .or_else(|| { t1.props.get("padding").and_then(|v| v.as_str()) }),
        Some("1px"),
        "plane omit → padding 1px (body or props)"
    );

    let right_rail = find_panel(panels, "right_rail").expect("right_rail");
    assert_eq!(
        right_rail.layout.as_ref().and_then(|l| l.gap.as_deref()),
        Some("1px"),
        "region omit → section outer margin 1px"
    );
    let region_pad = right_rail
        .body_props
        .get("padding")
        .and_then(|v| v.as_str())
        .or_else(|| right_rail.props.get("padding").and_then(|v| v.as_str()));
    assert_eq!(region_pad, Some("1px"), "region omit → padding 1px");
    assert_eq!(
        right_rail.props.get("radius").and_then(|v| v.as_str()),
        Some("0"),
        "region omit → radius 0"
    );
    assert_eq!(
        right_rail.props.get("border").and_then(|v| v.as_str()),
        Some("none"),
        "region omit → border 0"
    );

    let warning = find_panel(panels, "warning").expect("warning section");
    assert_eq!(
        warning.layout.as_ref().and_then(|l| l.gap.as_deref()),
        Some("1px"),
        "section omit → inner grid outer margin 1px"
    );
    let section_pad = warning
        .body_props
        .get("padding")
        .and_then(|v| v.as_str())
        .or_else(|| warning.props.get("padding").and_then(|v| v.as_str()));
    assert_eq!(
        section_pad,
        Some("1px"),
        "section omit → padding 1px / space_1"
    );
    assert_eq!(
        warning.props.get("radius").and_then(|v| v.as_str()),
        Some("0"),
        "section omit → radius 0"
    );
    assert!(
        warning
            .props
            .get("border")
            .and_then(|v| v.as_str())
            .is_some_and(|b| b.starts_with("1px")),
        "section omit → border width 1px; got {:?}",
        warning.props.get("border")
    );

    let stats = find_panel(panels, "supervision-stats").expect("supervision-stats content");
    assert_eq!(
        stats.layout.as_ref().and_then(|l| l.gap.as_deref()),
        Some("0"),
        "leaf content omit → inner gap 0"
    );
    let content_pad = stats
        .body_props
        .get("padding")
        .and_then(|v| v.as_str())
        .or_else(|| stats.props.get("padding").and_then(|v| v.as_str()));
    assert_eq!(content_pad, Some("0"), "content grid omit → padding 0");
    assert_eq!(
        stats.layout.as_ref().and_then(|l| l.align.as_deref()),
        Some("stretch"),
        "content omit → align stretch"
    );
    assert_eq!(
        stats.layout.as_ref().and_then(|l| l.justify.as_deref()),
        Some("stretch"),
        "content omit → justify stretch"
    );

    let manifest = outcome
        .compiled
        .ui_layout_index
        .layout_budget_manifest("test");
    let t1_entry = manifest
        .entries
        .get("t1/t1")
        .expect("t1/t1 layout budget entry");
    assert_eq!(
        t1_entry.padding.as_deref(),
        Some("1px"),
        "plane padding must reach layout_budget_manifest for DOM projection; entry={t1_entry:?}"
    );
}

#[test]
fn mini_data_header_structure_includes_brand() {
    let outcome = mini_data_home_outcome();
    let structure = mei_host_graph::build_structure_full_document(&outcome.compiled, "test");
    let scopes: Vec<_> = structure
        .nodes
        .iter()
        .map(|n| n.preview_scope.as_str())
        .collect();
    assert!(
        scopes.iter().any(|s| s.contains("screen_header_brand")),
        "header chrome_role must still walk nested region → brand; scopes={scopes:#?}"
    );
    assert!(
        scopes.iter().any(|s| s.contains("home_header")),
        "expected home_header region scope; scopes={scopes:#?}"
    );
}
