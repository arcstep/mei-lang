use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;
use mei_lang_kernel::{UiNodeDecl, UiTreeNode};

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
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
fn zhifa_enforcement_section_uses_padding_profile_not_body_props() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let enforcement = find_panel_in_tree(panels, "enforcement").expect("enforcement section");
    assert_eq!(
        enforcement
            .props
            .get("__mei_padding_profile")
            .and_then(|v| v.as_str()),
        Some("dense_strip_100"),
        "section padding should come from padding_profile only: {:?}",
        enforcement.props,
    );
}
