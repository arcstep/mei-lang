//! Integration test: ws-demo-v2 import + assemble smoke.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, import_bundle, list_scope_routes, GraphNodeKind,
    ImportOptions, McgRegistryWriter,
};

static INIT: Once = Once::new();

fn ws_demo_v2_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2 workspace")
}

fn bundle_path() -> PathBuf {
    ws_demo_v2_root()
        .join("apps/data-demo/.mei/compile/data-demo.meibundle")
}

fn ensure_imported() -> PathBuf {
    let workspace = ws_demo_v2_root();
    INIT.call_once(|| {
        if !bundle_path().is_file() {
            panic!("run `mei-compiler compile --workspace ws-demo-v2 --app data-demo` first");
        }
        let ctx = HostContext::new(workspace.clone(), "data-demo");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle_path()),
            },
        )
        .expect("import bundle");
    });
    workspace
}

#[test]
fn ws_demo_v2_import_and_assemble_home() {
    let workspace = ensure_imported();
    let routes = list_scope_routes(workspace.as_path(), "data-demo").expect("routes");
    assert!(!routes.is_empty());

    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    assert_eq!(outcome.compiled.active_scene.as_deref(), Some("home"));
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    assert!(contract.frame.is_some(), "home frame should be lowered");
    assert_eq!(contract.panels.len(), 3, "home assembly references 3 panels");
    let block_count: usize = contract
        .panels
        .iter()
        .map(|panel| panel.blocks.len())
        .sum();
    assert!(block_count > 0, "home panels should contain blocks");
    assert!(
        !outcome.compiled.component_assets.is_empty(),
        "component assets should be loaded from workspace"
    );
}

#[test]
fn ws_demo_v2_home_page_renders_header_and_panel_titles() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let apps = vec![mei_lang_kernel::WorkspaceAppMeta {
        id: "data-demo".to_string(),
        title: outcome.compiled.title.clone(),
        root: outcome.compiled.app_root.clone(),
    }];
    let workspace = ensure_imported();
    let workspace_cfg = mei_lang_kernel::load_workspace_config(workspace.as_path());
    let theme_style =
        mei_lang_app::page_body_theme_style(&workspace_cfg, Some(&outcome.compiled), None);
    let html = mei_lang_app::render_page(
        &apps,
        &outcome.compiled,
        "data-demo",
        None,
        mei_lang_app::UiRouteMode::App,
        Some(outcome.compiled.active_target_file.as_str()),
        None,
        None,
        Some("home"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        None,
        &[],
        false,
        None,
        None,
        theme_style.as_str(),
        None,
        None,
    );
    assert!(
        html.contains("component-host") || html.contains("mei-cockpit-header-brand"),
        "home page should SSR cockpit header component; html_bytes={}",
        html.len()
    );
    assert!(
        html.contains("实时预警") || html.contains("panel-head-cell"),
        "home page should SSR titled shell headings; html_bytes={}",
        html.len()
    );
    assert!(
        html.contains("rgba(98,190,235,0.35)") || html.contains("98,190,235"),
        "home page should SSR solid_stack metric card border; html_bytes={}",
        html.len()
    );
    assert!(
        html.contains("__mei_runtime_ref") && html.contains("supervision_items_count"),
        "home page should inject metric runtime refs for client-side eval; html_bytes={}",
        html.len()
    );
}

#[test]
fn ws_demo_v2_board_semantic_ids_present() {
    let workspace = ensure_imported();
    let registry = McgRegistryWriter::load(workspace.as_path(), "data-demo");
    let assembly_keys: Vec<_> = registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::AssemblyView)
        .map(|n| n.id.key.clone())
        .collect();
    assert_eq!(assembly_keys.len(), 20, "expected 20 assembly_view/board keys");
    assert!(assembly_keys.iter().any(|k| k.contains("home@")));
}

#[test]
fn ws_demo_v2_all_board_scenes_assemble() {
    let workspace = ensure_imported();
    let scenes = mei_plug_ds::collect_all_board_scenes(workspace.as_path(), "data-demo");
    assert!(scenes.len() >= 20);
    for scene in scenes {
        let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", scene.as_str())
            .expect("assemble");
        assert!(outcome.is_some(), "missing assemble for scene {scene}");
    }
}
