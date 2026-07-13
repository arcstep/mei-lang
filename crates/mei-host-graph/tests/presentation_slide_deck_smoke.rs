use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

#[test]
fn mei_tutorial_intro_assembles_presentation_deck() {
    let root = workspace_root();
    let outcome = assemble_scope_from_registry(root.as_path(), "mei-tutorial", "intro")
        .expect("assemble")
        .expect("outcome");
    let deck = outcome
        .presentation_map
        .get("deck")
        .expect("deck in presentation_map");
    let slides = deck.get("slides").and_then(|v| v.as_array()).expect("slides");
    assert_eq!(slides.len(), 9, "expected 9 tutorial slides, got {slides:?}");
    assert_eq!(
        slides[0].get("id").and_then(|v| v.as_str()),
        Some("slide-01-cover")
    );
    assert!(
        outcome
            .compiled
            .scene_contract
            .as_ref()
            .map(|c| c.panels.iter().any(|p| {
                p.props
                    .get("__mei_ui_role")
                    .and_then(|v| v.as_str())
                    == Some("plane")
                    || p.blocks.iter().any(|b| matches!(b, mei_lang_kernel::UiTreeNode::Panel(child) if child.props.get("__mei_ui_role").and_then(|v| v.as_str()) == Some("slide")))
            }))
            .unwrap_or(false),
        "expected plane/slide panels in contract"
    );
    let structure = &outcome.compiled.ui_layout_index;
    let slide_nodes: Vec<_> = structure
        .nodes
        .values()
        .filter(|n| n.role == mei_lang_kernel::UiScopeRole::Slide)
        .collect();
    assert_eq!(
        slide_nodes.len(),
        9,
        "structure index must emit UiScopeRole::Slide for each page"
    );
    let default_script = outcome
        .presentation_map
        .get("defaultScript")
        .or_else(|| outcome.presentation_map.get("default_script"))
        .expect("AOT defaultScript");
    let steps = default_script
        .get("steps")
        .and_then(|v| v.as_array())
        .expect("defaultScript.steps");
    assert!(
        steps.len() >= 9,
        "expected at least one step per slide, got {}",
        steps.len()
    );
    assert_eq!(
        default_script.get("id").and_then(|v| v.as_str()),
        Some("intro")
    );
}

#[test]
fn mini_data_supervision_assembles_four_slides() {
    let root = workspace_root();
    let outcome = assemble_scope_from_registry(root.as_path(), "mini-data", "supervision")
        .expect("assemble")
        .expect("outcome");
    let deck = outcome
        .presentation_map
        .get("deck")
        .expect("deck in presentation_map");
    let slides = deck.get("slides").and_then(|v| v.as_array()).expect("slides");
    assert_eq!(slides.len(), 4, "expected 4 supervision slides, got {slides:?}");
    assert_eq!(
        slides[0].get("id").and_then(|v| v.as_str()),
        Some("slide-01-mission")
    );
}
