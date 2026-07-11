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
        let bundle =
            workspace.join("apps/mini-park/env/current/build/exchange/mini-park.meibundle");
        assert!(bundle.is_file(), "compile mini-park first");
        let ctx = HostContext::new(workspace, "mini-park");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import mini-park bundle");
    });
}

fn has_panel_id(panel: &mei_lang_kernel::UiNodeDecl, target: &str) -> bool {
    if panel.id == target {
        return true;
    }
    panel.blocks.iter().any(|node| match node {
        mei_lang_kernel::UiTreeNode::Panel(child) => has_panel_id(child, target),
        _ => false,
    })
}

#[test]
fn mini_park_home_assembles_t2_pages_inside_scene_tree() {
    ensure_imported();
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "mini-park", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    assert!(
        panels
            .iter()
            .any(|panel| has_panel_id(panel, "park_point_1_page")),
        "expected T2 page inside home scene tree, got {:?}",
        panels
            .iter()
            .map(|panel| panel.id.clone())
            .collect::<Vec<_>>()
    );
}
