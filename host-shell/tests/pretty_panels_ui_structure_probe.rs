use std::path::PathBuf;
use std::sync::Once;
use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::build_ui_layout_index;

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(&ctx, &ImportOptions { bundle_path: Some(bundle) }).expect("import");
    });
    workspace
}

#[test]
fn pretty_panels_ui_structure_includes_left_rail_sections() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let left_rail = outcome.compiled.scene_contract.as_ref().unwrap().panels.iter().find(|p| p.id == "left_rail").expect("left_rail");
    eprintln!("left_rail blocks: {}", left_rail.blocks.len());
    for b in &left_rail.blocks {
        if let mei_lang_kernel::UiNodeDecl::Panel(p) = b {
            eprintln!("  section panel id={} title={:?} blocks={}", p.id, p.title, p.blocks.len());
        }
    }
    let ui = build_ui_layout_index(&outcome.compiled);
    let enforcement = ui.index.nodes.keys().find(|k| k.contains("enforcement"));
    eprintln!("ui nodes with enforcement: {:?}", enforcement);
    eprintln!("ui index node count: {}", ui.index.nodes.len());
    assert!(ui.index.nodes.keys().any(|k| k.contains("left_rail/enforcement")), "missing enforcement in ui index: {:?}", ui.index.nodes.keys().collect::<Vec<_>>());

    let enforcement_panel = outcome
        .compiled
        .scene_contract
        .as_ref()
        .unwrap()
        .panels
        .iter()
        .find(|p| p.id == "left_rail")
        .and_then(|rail| {
            rail.blocks.iter().find_map(|block| match block {
                mei_lang_kernel::UiNodeDecl::Panel(section) if section.id == "enforcement" => {
                    Some(section)
                }
                _ => None,
            })
        })
        .expect("enforcement section panel");
    assert!(
        !enforcement_panel.blocks.is_empty(),
        "enforcement section should contain lowered metric blocks"
    );
}

#[test]
fn pretty_panels_assemble_accepts_legacy_assembly_scene_id() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "assembly")
        .expect("assemble")
        .expect("home via assembly alias");
    assert_eq!(
        outcome.compiled.active_scene.as_deref(),
        Some("home"),
        "legacy scene id assembly should resolve to home"
    );
}
