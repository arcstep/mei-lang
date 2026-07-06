use std::path::PathBuf;

use mei_host_core::HostContext;
use mei_host_graph::linked_board_scenes_for_scope;

fn ws_demo_v2_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2 workspace")
}

#[test]
fn ws_demo_home_neighbor_sections_are_linked() {
    let workspace = ws_demo_v2_root();
    let ctx = HostContext::new(workspace, "data-demo".to_string());
    let linked = linked_board_scenes_for_scope(&ctx, "home", 1).expect("linked scenes");
    assert!(
        linked.iter().any(|scope| scope.contains("s-inspection-dashboard")),
        "expected inspection dashboard neighbor, got {linked:?}"
    );
    assert!(
        linked.iter().any(|scope| scope.contains("s-supervision-warning")),
        "expected supervision-warning neighbor, got {linked:?}"
    );
}
