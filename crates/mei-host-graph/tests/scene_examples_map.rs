use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_imported() {
    INIT.call_once(|| {
        let workspace = ws_demo_v2();
        let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
        assert!(bundle.is_file(), "compile data-demo first");
        let ctx = HostContext::new(workspace, "data-demo");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import data-demo bundle");
    });
}

#[test]
fn data_demo_scene_examples_by_id_matches_page_instance() {
    ensure_imported();
    let workspace = ws_demo_v2();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let scene_id = "enforcement_units_analytics_page";
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
    let assembly = outcome
        .compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .expect("assembly in projection map");
    let slots = assembly
        .get("projection_slots")
        .and_then(|v| v.as_array())
        .filter(|items| !items.is_empty())
        .expect("projection_slots array");
    for slot in slots {
        assert!(
            slot.get("layout_zone")
                .or_else(|| slot.get("layoutZone"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .is_some(),
            "slot missing layout_zone: {slot}"
        );
    }
    let shell_zones = assembly
        .get("shell_contract")
        .and_then(|s| s.get("zones"))
        .and_then(|z| z.as_array())
        .filter(|items| !items.is_empty());
    assert!(shell_zones.is_some(), "shell_contract.zones for {scene_id}");
    assert_eq!(
        assembly
            .get("shell_contract")
            .and_then(|s| s.get("layout_mode"))
            .and_then(|v| v.as_str()),
        Some("analytics"),
        "shell_contract.layout_mode for {scene_id}"
    );
}
