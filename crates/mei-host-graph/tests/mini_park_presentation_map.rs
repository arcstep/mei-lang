//! Regression: mini-park basemap view_ref viewpoints land in presentation_map.

use std::path::PathBuf;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

#[test]
fn mini_park_presentation_map_includes_basemap_viewpoints() {
    let workspace = ws_demo_v2();
    let bundle = workspace.join("apps/mini-park/env/current/build/exchange/mini-park.meibundle");
    assert!(
        bundle.is_file(),
        "run mei-compiler compile --workspace ws-demo-v2 --app mini-park"
    );
    let ctx = HostContext::new(workspace.clone(), "mini-park");
    import_bundle(
        &ctx,
        &ImportOptions {
            bundle_path: Some(bundle.clone()),
        },
    )
    .expect("import mini-park");
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble")
        .expect("home outcome");
    let viewpoints = outcome
        .presentation_map
        .get("viewpoints")
        .and_then(|v| v.as_object())
        .expect("viewpoints object");
    assert!(
        viewpoints.contains_key("park_point_1_entry"),
        "presentation_map keys: {:?}",
        viewpoints.keys().collect::<Vec<_>>()
    );
    let entry = viewpoints.get("park_point_1_entry").expect("entry");
    assert_eq!(
        entry.get("worldRef").and_then(|v| v.as_str()),
        Some("park_world")
    );
}
