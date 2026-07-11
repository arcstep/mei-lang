use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    import_bundle, linked_t2_page_pack_scopes, linked_t2_page_scenes_for_scope,
    t2_page_scenes_for_section_scope, ImportOptions,
};

static INIT: Once = Once::new();

fn ws_demo_v2_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2 workspace")
}

fn ensure_imported() {
    INIT.call_once(|| {
        let workspace = ws_demo_v2_root();
        let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
        assert!(bundle.is_file(), "compile data-demo first");
        let ctx = HostContext::new(workspace, "data-demo".to_string());
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
fn ws_demo_home_neighbor_sections_are_linked() {
    ensure_imported();
    let workspace = ws_demo_v2_root();
    let ctx = HostContext::new(workspace, "data-demo".to_string());
    let linked = linked_t2_page_scenes_for_scope(&ctx, "home", 1).expect("linked scenes");
    assert!(
        linked
            .iter()
            .any(|scope| scope.contains("s-inspection-dashboard")),
        "expected inspection dashboard neighbor, got {linked:?}"
    );
    assert!(
        linked
            .iter()
            .any(|scope| scope.contains("s-supervision-warning")),
        "expected supervision-warning neighbor, got {linked:?}"
    );
}

#[test]
fn ws_demo_collect_all_board_includes_penalty_total() {
    ensure_imported();
    let workspace = ws_demo_v2_root();
    let all = mei_host_graph::collect_all_t2_page_scenes(workspace.as_path(), "data-demo");
    assert!(
        all.iter()
            .any(|scene| scene == "penalty_total_analytics_page"),
        "collect_all_t2_page_scenes missing penalty_total, got {} scenes sample={:?}",
        all.len(),
        all.iter().take(10).collect::<Vec<_>>()
    );
}

#[test]
fn ws_demo_penalty_section_maps_to_page_scenes() {
    ensure_imported();
    let workspace = ws_demo_v2_root();
    let ctx = HostContext::new(workspace.clone(), "data-demo".to_string());
    let pages = t2_page_scenes_for_section_scope(
        workspace.as_path(),
        "data-demo",
        "home/t2/r-drilldown/s-penalty-dashboard",
    );
    assert!(
        pages
            .iter()
            .any(|scene| scene == "penalty_total_analytics_page"),
        "expected penalty_total_analytics_page, got {pages:?}"
    );
    let pack = linked_t2_page_pack_scopes(&ctx, "home", 1, 8).expect("pack scopes");
    assert!(
        pack.iter()
            .any(|scope| scope == "penalty_total_analytics_page"),
        "expected page scene in pack scopes, got {pack:?}"
    );
}

#[test]
fn ws_demo_home_bootstrap_payload_includes_t2_neighbor_scopes() {
    let workspace = ws_demo_v2_root();
    let manifest_dir = workspace.join("apps/data-demo/env/current/var/client-bootstrap");
    if !manifest_dir.is_dir() {
        eprintln!("skip: run data-demo prebuild to populate client-bootstrap manifests");
        return;
    }
    let mut scope_ids = Vec::new();
    for entry in std::fs::read_dir(&manifest_dir).expect("read client-bootstrap dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if !stem.is_empty() {
            scope_ids.push(stem);
        }
    }
    scope_ids.sort();
    if scope_ids.len() < 3 {
        eprintln!(
            "skip: expected >=3 client-bootstrap manifests after prebuild, got {scope_ids:?}"
        );
        return;
    }
    assert!(
        scope_ids
            .iter()
            .any(|scope| scope == "penalty_total_analytics_page"),
        "expected penalty_total_analytics_page manifest on disk, got {scope_ids:?}"
    );
}
