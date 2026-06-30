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
        let bundle = workspace.join(
            "apps/mini-park/env/2.0.7-ws20260628/build/exchange/mini-park.meibundle",
        );
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

#[test]
fn mini_park_park_point_board_assembles_inline_panels() {
    ensure_imported();
    let outcome = assemble_scope_from_registry(
        ws_demo_v2().as_path(),
        "mini-park",
        "park_point_1_board",
    )
    .expect("assemble")
    .expect("park_point_1_board outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    assert!(
        panels.len() >= 2,
        "expected title/body panels, got {:?}",
        panels.iter().map(|panel| panel.id.clone()).collect::<Vec<_>>()
    );
}
