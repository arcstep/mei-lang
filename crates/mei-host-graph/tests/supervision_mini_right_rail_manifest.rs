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
