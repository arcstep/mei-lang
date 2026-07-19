use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;

fn workspace_root() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

#[test]
fn mei_tutorial_intro_assembles_presentation_deck() {
    let Some(root) = workspace_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(root.as_path(), "mei-tutorial", "intro")
        .expect("assemble")
        .expect("outcome");
    let deck = outcome
        .presentation_map
        .get("deck")
        .expect("deck in presentation_map");
    let slides = deck
        .get("slides")
        .and_then(|v| v.as_array())
        .expect("slides");
    assert_eq!(
        slides.len(),
        9,
        "expected 9 tutorial slides, got {slides:?}"
    );
    assert_eq!(
        slides[0].get("id").and_then(|v| v.as_str()),
        Some("slide-01-cover")
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
    assert!(
        outcome.presentation_map.get("defaultScript").is_none()
            && outcome.presentation_map.get("default_script").is_none(),
        "deck/legacy scene sources must not synthesize an AOT defaultScript"
    );
}

#[test]
fn mini_data_supervision_assembles_four_slides() {
    let Some(root) = workspace_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(root.as_path(), "mini-data", "supervision")
        .expect("assemble")
        .expect("outcome");
    let deck = outcome
        .presentation_map
        .get("deck")
        .expect("deck in presentation_map");
    let slides = deck
        .get("slides")
        .and_then(|v| v.as_array())
        .expect("slides");
    assert_eq!(
        slides.len(),
        4,
        "expected 4 supervision slides, got {slides:?}"
    );
    assert_eq!(
        slides[0].get("id").and_then(|v| v.as_str()),
        Some("slide-01-mission")
    );
}
