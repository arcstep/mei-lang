//! Platform conformance: T2 catalog + scene examples + import/assemble smoke.

use mei_host_graph::assemble_scope_from_registry;
use mei_test_support::{ensure_imported, APP_STRUCTURE};

#[test]
fn conformance_t2_pages_in_catalog() {
    let workspace = ensure_imported(APP_STRUCTURE);
    let outcome = assemble_scope_from_registry(workspace.as_path(), APP_STRUCTURE, "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    assert!(
        !contract.scene.t2_pages.is_empty(),
        "expected auto-discovered t2_pages, got empty; panels={:?}",
        contract
            .panels
            .iter()
            .map(|panel| panel.id.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        contract.panels.iter().all(|panel| {
            let tier = panel
                .props
                .get("__mei_tier")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            tier != "t2"
        }),
        "t2 page-planes must not mount into always-on panels"
    );
}

#[test]
fn conformance_scene_examples_map() {
    let workspace = ensure_imported(APP_STRUCTURE);
    let outcome = assemble_scope_from_registry(workspace.as_path(), APP_STRUCTURE, "home")
        .expect("assemble")
        .expect("home outcome");
    let scene_id = "fx_analytics_page";
    let examples = outcome
        .compiled
        .scene_examples_by_id
        .get(scene_id)
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "expected scene_examples_by_id[{scene_id}], keys={:?}",
                outcome
                    .compiled
                    .scene_examples_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let assembly_examples = outcome
        .compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(|assembly| assembly.get("examples"))
        .cloned()
        .unwrap_or_else(|| panic!("expected assembly examples for {scene_id}"));
    assert_eq!(
        examples, assembly_examples,
        "scene_examples_by_id should mirror page_instance.examples"
    );
    assert!(
        !examples.is_null(),
        "examples payload should not be null for {scene_id}"
    );
}

#[test]
fn conformance_import_assemble_home() {
    let workspace = ensure_imported(APP_STRUCTURE);
    let outcome = assemble_scope_from_registry(workspace.as_path(), APP_STRUCTURE, "home")
        .expect("assemble")
        .expect("home outcome");
    assert!(
        outcome.compiled.scene_contract.is_some(),
        "home assemble should produce scene_contract"
    );
}
