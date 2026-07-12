use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};
use mei_lang_kernel::{UiNodeDecl, UiTreeNode};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_pretty_panels_imported() {
    INIT.call_once(|| {
        let workspace = ws_demo_v2();
        let bundle =
            workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        if !bundle.is_file() {
            return;
        }
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import pretty-panels bundle");
        clear_assemble_cache_for_app("pretty-panels");
    });
}

fn find_panel<'a>(panel: &'a UiNodeDecl, id: &str) -> Option<&'a UiNodeDecl> {
    if panel.id == id {
        return Some(panel);
    }
    for block in &panel.blocks {
        if let UiTreeNode::Panel(nested) = block {
            if let Some(found) = find_panel(nested, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_panel_in_tree<'a>(panels: &'a [UiNodeDecl], id: &str) -> Option<&'a UiNodeDecl> {
    panels.iter().find_map(|panel| find_panel(panel, id))
}

#[test]
fn pretty_panels_enforcement_body_includes_triptych_and_compound_shell() {
    ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let strip =
        find_panel_in_tree(panels, "enforcement_strip_layout").expect("enforcement_strip_layout");
    let areas = strip
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
        .expect("enforcement_strip_layout grid areas");
    assert!(
        areas
            .iter()
            .flatten()
            .any(|area| area == "first" || area == "compound"),
        "enforcement_strip_layout should include triptych and compound areas, got {areas:?}"
    );
    let compound =
        find_panel_in_tree(panels, "enforcement_strip_layout_compound").expect("compound card");
    let bg_json = serde_json::to_string(compound.props.get("background").expect("background"))
        .unwrap_or_default();
    assert!(
        bg_json.contains("metric-bg-target"),
        "compound slot should keep metric-bg-target frame, got {bg_json}"
    );
}

#[test]
fn pretty_panels_issue_body_exports_four_status_metric_cards() {
    ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    for suffix in ["_pending", "_doing", "_done", "_summary"] {
        let card_id = format!("issue_body{suffix}_content");
        let card = find_panel_in_tree(panels, card_id.as_str())
            .unwrap_or_else(|| panic!("missing issue metric card {card_id}"));
        assert_eq!(
            card.props
                .get("__mei_metric_template")
                .and_then(|v| v.as_str()),
            Some(if suffix == "_summary" { "row" } else { "stack" }),
            "issue card {card_id} template mismatch: {:?}",
            card.props
        );
    }
    // Icon + slot-fill layers live on the shell panel, not the inner metric card.
    let summary_shell = find_panel_in_tree(panels, "issue_body_summary").expect("summary shell");
    let images = summary_shell
        .props
        .get("background")
        .and_then(|bg| bg.get("image"))
        .and_then(|v| v.as_array())
        .expect("summary shell multilayer background.image");
    assert!(
        images.iter().any(|v| {
            v.as_str()
                .is_some_and(|s| s.contains("url(") && !s.contains("linear-gradient"))
        }),
        "summary shell should keep icon/slot-fill layers, got {images:?}"
    );
    assert_eq!(
        summary_shell
            .props
            .get("__mei_slot_frame_bg")
            .and_then(|v| v.as_bool()),
        Some(true),
        "summary shell should keep slot-frame flag"
    );
}
