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
        .join("apps/data-demo/build/active/exchange/data-demo.meibundle")
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
fn ws_demo_v2_build_store_layout() {
    let workspace = ws_demo_v2_root();
    let active = workspace.join("apps/data-demo/build/active");
    if !active.exists() {
        return;
    }
    #[cfg(unix)]
    {
        assert!(
            active.is_symlink(),
            "build/active should be a symlink to env/{{ver}}/build after prebuild"
        );
        let target = std::fs::read_link(&active).expect("read build/active symlink");
        assert!(
            target.to_string_lossy().contains("/env/"),
            "build/active should point under env/{{ver}}/build, got {}",
            target.display()
        );
        let env_ver = target
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let manifest = workspace
            .join("apps/data-demo/env")
            .join(env_ver)
            .join("BUILD.json");
        assert!(
            manifest.is_file(),
            "BUILD.json missing at {}",
            manifest.display()
        );
        let registry = workspace
            .join("apps/data-demo/build/active/registry/mcg-registry.json");
        assert!(registry.is_file(), "registry should live under build/active");
    }
}

#[test]
fn ws_demo_v2_upload_dir_separate_from_assets() {
    let workspace = ws_demo_v2_root();
    let upload = workspace.join("apps/data-demo/upload");
    if !upload.is_dir() {
        return;
    }
    let app_config = workspace.join("apps/data-demo/app.config.json");
    let raw = std::fs::read_to_string(app_config).expect("read app.config");
    assert!(
        raw.contains(r#""upload": "upload""#),
        "app.config paths.upload should be upload/"
    );
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
    assert_eq!(contract.panels.len(), 6, "home assembly references 6 panels");
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
fn ws_demo_v2_home_contract_expands_rail_metric_panels() {
    let workspace = ws_demo_v2_root();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    fn panel_paths(panel: &mei_lang_kernel::PanelDecl, prefix: &str, out: &mut Vec<String>) {
        let path = if prefix.is_empty() {
            panel.id.clone()
        } else {
            format!("{prefix}/{}", panel.id)
        };
        out.push(path.clone());
        for node in &panel.blocks {
            if let mei_lang_kernel::UiNodeDecl::Panel(nested) = node {
                panel_paths(nested, path.as_str(), out);
            }
        }
    }
    let mut paths = Vec::new();
    for panel in &contract.panels {
        panel_paths(panel, "", &mut paths);
    }
    assert!(
        paths.iter().any(|p| p.contains("supervision-stats")),
        "home contract should expand rail slot panel_ref; got paths={paths:?}"
    );
}

#[test]
fn ws_demo_v2_home_gis_map_spec_resolves_config_refs() {
    let workspace = ws_demo_v2_root();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");

    fn find_map_spec_in_panel(panel: &mei_lang_kernel::PanelDecl) -> Option<serde_json::Value> {
        for node in &panel.blocks {
            match node {
                mei_lang_kernel::UiNodeDecl::Block(block) if block.use_key == "map.maplibre" => {
                    return block.props.get("mapSpec").cloned();
                }
                mei_lang_kernel::UiNodeDecl::Panel(nested) => {
                    if let Some(spec) = find_map_spec_in_panel(nested) {
                        return Some(spec);
                    }
                }
                _ => {}
            }
        }
        None
    }

    let mut map_spec = None;
    for panel in &contract.panels {
        if let Some(spec) = find_map_spec_in_panel(panel) {
            map_spec = Some(spec);
            break;
        }
    }
    let map_spec = map_spec.expect("map.maplibre mapSpec should be lowered");
    assert!(
        !map_spec.to_string().contains("__var"),
        "mapSpec should not contain unresolved panel constants: {map_spec}"
    );
    let basemap = map_spec.get("basemap").expect("basemap");
    assert!(
        basemap.get("tilesUrl").is_some() || basemap.get("tilesJsonPath").is_some(),
        "basemap_ref should resolve to tiles config: {basemap}"
    );
    let first_layer_url = map_spec["layers"]
        .as_array()
        .and_then(|layers| layers.first())
        .and_then(|layer| layer.get("url"))
        .expect("first layer url");
    assert!(
        first_layer_url.as_str().is_some_and(|url| url.starts_with('/')),
        "ops_param_ref layer url should resolve to path string: {first_layer_url}"
    );

    let header = contract
        .panels
        .iter()
        .find(|panel| panel.id == "home_header")
        .expect("home_header panel");
    assert_eq!(
        header.props.get("z_index").and_then(serde_json::Value::as_i64),
        Some(110)
    );
}

#[test]
fn ws_demo_v2_serve_style_render_includes_rail_metric_panels() {
    let workspace = ws_demo_v2_root();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let app_root =
        mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
    let apps = vec![mei_lang_kernel::WorkspaceAppMeta {
        id: "data-demo".to_string(),
        title: outcome.compiled.title.clone(),
        root: app_root.display().to_string(),
    }];
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
        html.contains("supervision-stats"),
        "serve-style render should include nested supervision-stats; mei-text={}",
        html.matches("<mei-text").count()
    );
    assert!(
        html.matches("<mei-text").count() >= 10,
        "serve-style render should SSR metric slots, got {} mei-text",
        html.matches("<mei-text").count()
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
        html.contains("data-mei-head-carets=\"true\"") || html.contains("--mei-head-caret-url"),
        "home page should SSR panel title carets"
    );
    assert!(
        html.contains("background-image:var(--mei-gradient-panel-title-bar)")
            || html.contains("--mei-gradient-panel-title-bar"),
        "home page should include panel title bar gradient token"
    );
    assert!(
        html.contains("/workspace-app-assets/data-demo/assets/header/screen-title-bg"),
        "home header should resolve screen title background asset"
    );
    assert!(
        html.matches("<mei-text").count() >= 10,
        "home should SSR metric card mei.text slots, got {} mei-text tags",
        html.matches("<mei-text").count()
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
    assert_eq!(assembly_keys.len(), 43, "expected 43 assembly_view/board keys");
    assert!(assembly_keys.iter().any(|k| k.contains("home@")));
}

#[test]
fn ws_demo_v2_all_board_scenes_assemble() {
    let workspace = ensure_imported();
    let scenes = mei_plug_ds::collect_all_board_scenes(workspace.as_path(), "data-demo");
    assert!(scenes.len() >= 43);
    for scene in scenes {
        let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", scene.as_str())
            .expect("assemble");
        assert!(outcome.is_some(), "missing assemble for scene {scene}");
    }
}

#[test]
fn ws_demo_v2_assemble_without_reimport() {
    let workspace = ws_demo_v2_root();
    if !bundle_path().is_file() {
        return;
    }
    let result = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home");
    match &result {
        Ok(Some(_)) => {}
        Ok(None) => panic!("assemble returned None without reimport"),
        Err(e) => panic!("assemble error: {e:#}"),
    }
}

#[test]
fn ws_demo_v2_assemble_relative_workspace_path() {
    let rel = std::path::PathBuf::from("../workspaces/ws-demo-v2");
    if !rel.join("apps/data-demo/build/active/exchange/data-demo.meibundle").is_file() {
        return;
    }
    let result = assemble_scope_from_registry(rel.as_path(), "data-demo", "home");
    match &result {
        Ok(Some(_)) => {}
        Ok(None) => panic!("assemble None with relative workspace path"),
        Err(e) => panic!("assemble error with relative path: {e:#}"),
    }
}

#[test]
fn ws_demo_v2_warmup_tier_all_populates_mrg_and_memory_hit() {
    let workspace = ensure_imported();
    let ctx = HostContext::new(workspace.clone(), "data-demo".to_string());
    let targets =
        mei_plug_ds::collect_warmup_targets(&ctx, Some("home")).expect("warmup targets");
    assert!(!targets.is_empty(), "home warmup policy should define targets");
    let report = mei_plug_ds::run_warmup_targets_with_tier(
        &ctx,
        &targets,
        mei_plug_ds::WarmupTier::All,
    )
    .expect("warmup tier all");
    assert!(report.slot_count > 0, "warmup should register MRG slots");
    let status =
        mei_host_graph::mrg_status_json(workspace.as_path(), "data-demo").expect("mrg status");
    let memory_resident = status
        .get("slotsByTier")
        .and_then(|value| value.get("memoryResident"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    assert!(
        memory_resident > 0,
        "tier all should pin at least one memory resident slot"
    );

    let target = &targets[0];
    let (compiled, compile_revision) =
        mei_plug_ds::load_compiled_for_warmup(&ctx, target.scope_key.as_str()).expect("compiled");
    let outcome = mei_plug_ds::eval_metric_ids(
        &ctx,
        &compiled,
        compile_revision.as_str(),
        target.scope_key.as_str(),
        target.owner_resource_id.as_str(),
        target.workset_id.as_str(),
        target.bundle_key.as_str(),
        &target.metric_ids,
    )
    .expect("second eval");
    assert!(
        outcome.artifact_hit,
        "second eval should hit in-memory metric response cache"
    );
}
