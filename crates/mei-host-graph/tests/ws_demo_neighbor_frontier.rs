use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    import_bundle, linked_t2_page_pack_scopes, linked_t2_page_scenes_for_scope,
    t2_page_scenes_for_section_scope, ImportOptions,
};

static INIT: Once = Once::new();

fn ws_demo_v2_root() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn skip_if_data_demo_missing() -> Option<PathBuf> {
    let workspace = ws_demo_v2_root()?;
    if !workspace.join("apps/data-demo").is_dir() {
        return None;
    }
    Some(workspace)
}

fn ensure_imported() -> Option<PathBuf> {
    let workspace = skip_if_data_demo_missing()?;
    INIT.call_once(|| {
        let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
        if !bundle.is_file() {
            return;
        }
        let ctx = HostContext::new(workspace.clone(), "data-demo".to_string());
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import data-demo bundle");
    });
    let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
    if !bundle.is_file() {
        return None;
    }
    Some(workspace)
}

#[test]
fn ws_demo_home_neighbor_sections_are_linked() {
    let Some(workspace) = ensure_imported() else {
        return;
    };
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
    let Some(workspace) = ensure_imported() else {
        return;
    };
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
    let Some(workspace) = ensure_imported() else {
        return;
    };
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
    let Some(workspace) = ws_demo_v2_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
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
