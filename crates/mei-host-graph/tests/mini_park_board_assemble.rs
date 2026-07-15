use std::path::PathBuf;

use mei_bundle::{compute_workspace_digest, write_bundle_from_outcome};
use mei_graph::compile_app;
use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn ensure_imported() -> Option<PathBuf> {
    let workspace = ws_demo_v2()?;
    let outcome = compile_app(workspace.as_path(), "mini-park").expect("compile mini-park");
    let digest = compute_workspace_digest(workspace.as_path(), "mini-park", "stock/templates");
    let temp_dir = std::env::temp_dir().join("mei-mini-park-board-assemble");
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let bundle_path = temp_dir.join("mini-park.meibundle");
    write_bundle_from_outcome(
        &outcome,
        digest.as_str(),
        env!("CARGO_PKG_VERSION"),
        bundle_path.as_path(),
        false,
    )
    .expect("write bundle");
    let ctx = HostContext::new(workspace.clone(), "mini-park");
    import_bundle(
        &ctx,
        &ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )
    .expect("import mini-park bundle");
    clear_assemble_cache_for_app("mini-park");
    Some(workspace)
}

#[test]
fn mini_park_home_assembles_t2_pages_in_catalog() {
    let Some(workspace) = ensure_imported() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    // 0335: T2 page-planes live in catalog (scene.t2_pages), not always-on panels.
    assert!(
        !contract.scene.t2_pages.is_empty(),
        "expected auto-discovered t2_pages, got empty; always-on panels={:?}",
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
