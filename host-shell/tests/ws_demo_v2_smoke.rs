//! Integration test: ws-demo-v2 import + assemble smoke.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, collect_all_board_scenes, import_bundle, list_scope_routes,
    GraphNodeKind, ImportOptions, McgRegistryWriter,
};

static INIT: Once = Once::new();

fn ws_demo_v2_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2 workspace")
}

fn bundle_path() -> PathBuf {
    ws_demo_v2_root().join("apps/data-demo/build/active/exchange/data-demo.meibundle")
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
        let registry = workspace.join("apps/data-demo/build/active/registry/mcg-registry.json");
        assert!(
            registry.is_file(),
            "registry should live under build/active"
        );
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
    assert_eq!(
        contract.panels.len(),
        6,
        "home assembly references 6 panels"
    );
    let block_count: usize = contract.panels.iter().map(|panel| panel.blocks.len()).sum();
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
        first_layer_url
            .as_str()
            .is_some_and(|url| url.starts_with('/')),
        "ops_param_ref layer url should resolve to path string: {first_layer_url}"
    );

    let header = contract
        .panels
        .iter()
        .find(|panel| panel.id == "home_header")
        .expect("home_header panel");
    assert_eq!(
        header
            .props
            .get("z_index")
            .and_then(serde_json::Value::as_i64),
        Some(mei_host_graph::Z_T1_HEADER)
    );
}

#[test]
fn ws_demo_v2_serve_style_render_includes_rail_metric_panels() {
    let workspace = ws_demo_v2_root();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
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
    assert_eq!(
        assembly_keys.len(),
        43,
        "expected 43 assembly_view/board keys"
    );
    assert!(assembly_keys.iter().any(|k| k.contains("home@")));
}

#[test]
fn ws_demo_v2_all_board_scenes_assemble() {
    let workspace = ensure_imported();
    let scenes = collect_all_board_scenes(workspace.as_path(), "data-demo");
    assert!(scenes.len() >= 43);
    for scene in scenes {
        let outcome =
            assemble_scope_from_registry(workspace.as_path(), "data-demo", scene.as_str())
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
    if !rel
        .join("apps/data-demo/build/active/exchange/data-demo.meibundle")
        .is_file()
    {
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
fn ws_demo_v2_home_layer_plan_and_presentation_map() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    assert_eq!(
        outcome
            .layer_plan
            .get("schemaVersion")
            .and_then(|v| v.as_str()),
        Some("mei-layer-plan-v1")
    );
    let basemap = outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t0"))
        .and_then(|v| v.as_array())
        .expect("t0 tier entries");
    assert!(
        basemap
            .iter()
            .any(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("map_stage")),
        "layer_plan t0 should include map_stage: {basemap:?}"
    );
    let map_stage = basemap
        .iter()
        .find(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("map_stage"))
        .expect("map_stage layer plan entry");
    assert_eq!(
        map_stage.get("viewFamily").and_then(|v| v.as_str()),
        Some("map")
    );
    assert_eq!(
        map_stage.get("stageKind").and_then(|v| v.as_str()),
        Some("map-stage")
    );
    let chrome = outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t1"))
        .and_then(|v| v.as_array())
        .expect("t1 tier entries");
    let chrome_ids: Vec<&str> = chrome
        .iter()
        .filter_map(|entry| entry.get("panelId").and_then(|v| v.as_str()))
        .collect();
    for expected in [
        "home_header",
        "left_rail",
        "center_top",
        "realtime_center",
        "right_rail",
    ] {
        assert!(
            chrome_ids.contains(&expected),
            "layer_plan t1 should include {expected}: {chrome_ids:?}"
        );
    }
    assert_eq!(
        outcome
            .presentation_map
            .get("schemaVersion")
            .and_then(|v| v.as_str()),
        Some("mei-presentation-map-v1")
    );
    assert!(
        outcome.overlay_defaults.len() >= 36,
        "overlay_defaults should include all link_decl entries: {}",
        outcome.overlay_defaults.len()
    );
}

#[test]
fn ws_demo_v2_home_panels_emit_tier_props() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    let map_stage = contract
        .panels
        .iter()
        .find(|panel| panel.id == "map_stage")
        .expect("map_stage panel");
    assert_eq!(
        map_stage.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t0")
    );
    assert_eq!(
        map_stage
            .props
            .get("__mei_view_family")
            .and_then(|v| v.as_str()),
        Some("map")
    );
    assert_eq!(
        map_stage
            .props
            .get("__mei_stage_kind")
            .and_then(|v| v.as_str()),
        Some("map-stage")
    );
    let header = contract
        .panels
        .iter()
        .find(|panel| panel.id == "home_header")
        .expect("home_header panel");
    assert_eq!(
        header.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t1")
    );
    assert_eq!(
        header
            .props
            .get("z_index")
            .and_then(serde_json::Value::as_i64),
        Some(mei_host_graph::Z_T1_HEADER)
    );
}

#[test]
fn ws_demo_v2_serve_html_emits_data_mei_tier() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
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
        None,
    );
    assert!(
        html.contains("data-mei-tier=\"t0\""),
        "serve HTML should emit data-mei-tier for t0 panel"
    );
    assert!(
        html.contains("data-mei-tier=\"t1\""),
        "serve HTML should emit data-mei-tier for t1 panels"
    );
}

fn collect_lowered_viewpoint_ids(panels: &[mei_lang_kernel::PanelDecl]) -> Vec<String> {
    let mut found = Vec::new();
    fn walk(nodes: &[mei_lang_kernel::UiNodeDecl], found: &mut Vec<String>) {
        for node in nodes {
            match node {
                mei_lang_kernel::UiNodeDecl::Panel(panel) => {
                    if let Some(vp) = panel.props.get("__mei_viewpoint").and_then(|v| v.as_str()) {
                        found.push(vp.to_string());
                    }
                    walk(&panel.blocks, found);
                }
                mei_lang_kernel::UiNodeDecl::Block(_) => {}
                mei_lang_kernel::UiNodeDecl::PanelRefEmbed(_) => {}
            }
        }
    }
    for panel in panels {
        if let Some(vp) = panel.props.get("__mei_viewpoint").and_then(|v| v.as_str()) {
            found.push(vp.to_string());
        }
        walk(&panel.blocks, &mut found);
    }
    found
}

#[test]
fn ws_demo_v2_presentation_map_viewpoints() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let lowered = outcome
        .compiled
        .scene_contract
        .as_ref()
        .map(|contract| collect_lowered_viewpoint_ids(&contract.panels))
        .unwrap_or_default();
    let viewpoints = outcome
        .presentation_map
        .get("viewpoints")
        .and_then(|v| v.as_object())
        .expect("presentation_map viewpoints");
    for expected in [
        "warnings_total",
        "enforcement_stats",
        "inspection_stats",
        "penalty_stats",
        "indicator_system",
    ] {
        assert!(
            viewpoints.contains_key(expected),
            "presentation_map should include viewpoint {expected}: keys={:?}, lowered={lowered:?}",
            viewpoints.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn ws_demo_v2_serve_html_emits_data_mei_viewpoint() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "data-demo");
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
        None,
    );
    assert!(
        html.contains("data-mei-viewpoint=\"warnings_total\""),
        "serve HTML should emit data-mei-viewpoint for warnings_total"
    );
    assert!(
        html.contains("data-mei-viewpoint=\"enforcement_stats\""),
        "serve HTML should emit data-mei-viewpoint for enforcement_stats"
    );
}

#[test]
fn ws_demo_v2_discovers_data_demo_and_mini_park() {
    let workspace = ws_demo_v2_root();
    let apps = mei_lang_kernel::discover_apps(workspace.as_path()).expect("discover");
    let ids: Vec<&str> = apps.iter().map(|app| app.id.as_str()).collect();
    assert!(ids.contains(&"data-demo"), "discover apps: {ids:?}");
    assert!(ids.contains(&"mini-park"), "discover apps: {ids:?}");
}

#[test]
fn ws_demo_v2_topbar_renders_multi_app_menu_labels() {
    let workspace = ensure_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home outcome");
    let discovered = mei_lang_kernel::discover_apps(workspace.as_path()).expect("discover");
    let topbar_menu = mei_lang_app::load_topbar_menu_context(workspace.as_path());
    let apps: Vec<_> = discovered
        .iter()
        .map(|app| {
            let mut enriched = app.clone();
            if app.id == "data-demo" {
                enriched.title = "Data Demo v2".to_string();
            } else if app.id == "mini-park" {
                enriched.title = "迷你公园 · Mini Park".to_string();
            }
            enriched
        })
        .collect();
    let workspace_cfg = mei_lang_kernel::load_workspace_config(workspace.as_path());
    let theme_style =
        mei_lang_app::page_body_theme_style(&workspace_cfg, Some(&outcome.compiled), None);
    let html = mei_lang_app::render_page(
        &apps,
        &outcome.compiled,
        "data-demo",
        Some(&topbar_menu),
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
        None,
    );
    assert!(
        html.contains("Data Demo v2"),
        "topbar should list data-demo menu label"
    );
    assert!(
        html.contains("迷你公园"),
        "topbar should list mini-park menu label"
    );
    assert!(
        !html.contains(r#"data-topbar-menu-group="apps""#),
        "topbar should not force apps into aggregate group"
    );
    assert!(
        !html.contains(r#"data-topbar-menu-group="components""#),
        "stock components should not appear as topbar group"
    );
    assert!(
        !html.contains(r#"data-topbar-menu-group="templates""#),
        "stock templates should not appear as topbar group"
    );
}

fn ensure_mini_park_imported() -> PathBuf {
    let workspace = ws_demo_v2_root();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if bundle.is_file() {
        let ctx = HostContext::new(workspace.clone(), "mini-park");
        let _ = import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        );
    }
    workspace
}

#[test]
fn ws_demo_v2_mini_park_home_panels_emit_tier_props() {
    let workspace = ensure_mini_park_imported();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if !bundle.is_file() {
        return;
    }
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park")
        .expect("mini-park home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    let t0_panel = contract
        .panels
        .iter()
        .find(|panel| panel.props.get("__mei_tier").and_then(|v| v.as_str()) == Some("t0"))
        .expect("t0 panel");
    assert!(
        t0_panel.id == "basemap" || t0_panel.id == "viewport_canvas",
        "expected basemap or viewport_canvas as t0, got {}",
        t0_panel.id
    );
    let header = contract
        .panels
        .iter()
        .find(|panel| panel.id == "home_header")
        .expect("home_header panel");
    assert_eq!(
        header.props.get("__mei_tier").and_then(|v| v.as_str()),
        Some("t1")
    );
    assert_eq!(
        header
            .props
            .get("z_index")
            .and_then(serde_json::Value::as_i64),
        Some(mei_host_graph::Z_T1_HEADER)
    );
    let t0_tier = outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t0"))
        .and_then(|v| v.as_array())
        .expect("t0 tier entries");
    assert!(
        t0_tier.iter().any(|entry| {
            entry.get("panelId").and_then(|v| v.as_str()) == Some("basemap")
                || entry.get("panelId").and_then(|v| v.as_str()) == Some("viewport_canvas")
        }),
        "mini-park layer_plan t0 should include basemap or viewport_canvas: {t0_tier:?}"
    );
    if let Some(viewport_canvas) = t0_tier
        .iter()
        .find(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("viewport_canvas"))
    {
        assert_eq!(
            viewport_canvas.get("zIndex").and_then(|v| v.as_i64()),
            Some(mei_host_graph::default_z_index_for_tier(mei_host_graph::TIER_T0)),
            "viewport_canvas should be first T0 panel (z=1)"
        );
        assert_eq!(
            viewport_canvas.get("stackOrder").and_then(|v| v.as_u64()),
            Some(0)
        );
        assert_eq!(
            viewport_canvas.get("viewFamily").and_then(|v| v.as_str()),
            Some("canvas")
        );
        assert_eq!(
            viewport_canvas.get("stageKind").and_then(|v| v.as_str()),
            Some("viewport-canvas")
        );
    }
    if let Some(basemap) = t0_tier
        .iter()
        .find(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("basemap"))
    {
        assert_eq!(
            basemap.get("zIndex").and_then(|v| v.as_i64()),
            Some(mei_host_graph::default_z_index_for_tier(mei_host_graph::TIER_T0) + 1),
            "basemap should follow viewport_canvas in assembly order (z=2)"
        );
        assert_eq!(basemap.get("stackOrder").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(
            basemap.get("viewFamily").and_then(|v| v.as_str()),
            Some("map")
        );
        assert_eq!(
            basemap.get("stageKind").and_then(|v| v.as_str()),
            Some("map-stage")
        );
    }
    if let Some(world_viewport) = t0_tier
        .iter()
        .find(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("world_viewport"))
    {
        assert_eq!(
            world_viewport.get("zIndex").and_then(|v| v.as_i64()),
            Some(mei_host_graph::default_z_index_for_tier(mei_host_graph::TIER_T0) + 2),
            "world_viewport should be third T0 panel (z=3)"
        );
        assert_eq!(
            world_viewport.get("stackOrder").and_then(|v| v.as_u64()),
            Some(2)
        );
    }
    let viewpoints = outcome
        .presentation_map
        .get("viewpoints")
        .and_then(|v| v.as_object())
        .expect("mini-park presentation_map viewpoints");
    let overview = viewpoints
        .get("park_overview_stage")
        .expect("park_overview_stage viewpoint");
    assert_eq!(
        overview.get("viewFamily").and_then(|v| v.as_str()),
        Some("map")
    );
    assert_eq!(
        overview.get("panelId").and_then(|v| v.as_str()),
        Some("basemap")
    );
    assert_eq!(
        overview.get("stageKind").and_then(|v| v.as_str()),
        Some("map-stage")
    );
    assert_eq!(
        overview.get("worldRef").and_then(|v| v.as_str()),
        Some("park_world")
    );
    assert_eq!(
        overview.get("groupId").and_then(|v| v.as_str()),
        Some("park_story_overview")
    );
    assert_eq!(
        overview.get("cameraPreset").and_then(|v| v.as_str()),
        Some("park_overview_orbit")
    );
    let point_one = viewpoints
        .get("park_point_1_entry")
        .expect("park_point_1_entry viewpoint");
    assert_eq!(point_one.get("panelId").and_then(|v| v.as_str()), Some("basemap"));
    assert_eq!(point_one.get("viewFamily").and_then(|v| v.as_str()), Some("map"));
    assert_eq!(point_one.get("stageKind").and_then(|v| v.as_str()), Some("map-stage"));
    assert_eq!(point_one.get("worldRef").and_then(|v| v.as_str()), Some("park_world"));
    assert_eq!(
        point_one.get("entityId").and_then(|v| v.as_str()),
        Some("lake_pavilion")
    );
    assert_eq!(
        point_one.get("cameraPreset").and_then(|v| v.as_str()),
        Some("lake_pavilion_focus")
    );
}

#[test]
fn ws_demo_v2_mini_park_serve_html_emits_view_family_attrs() {
    let workspace = ensure_mini_park_imported();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if !bundle.is_file() {
        return;
    }
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park")
        .expect("mini-park home outcome");
    let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
    let apps = vec![mei_lang_kernel::WorkspaceAppMeta {
        id: "mini-park".to_string(),
        title: outcome.compiled.title.clone(),
        root: app_root.display().to_string(),
    }];
    let workspace_cfg = mei_lang_kernel::load_workspace_config(workspace.as_path());
    let theme_style =
        mei_lang_app::page_body_theme_style(&workspace_cfg, Some(&outcome.compiled), None);
    let html = mei_lang_app::render_page(
        &apps,
        &outcome.compiled,
        "mini-park",
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
        None,
    );
    assert!(
        html.contains("data-mei-view-family=\"map\""),
        "mini-park HTML should expose map stage family"
    );
    assert!(
        html.contains("data-mei-stage-kind=\"map-stage\""),
        "mini-park HTML should expose map stage kind"
    );
    assert!(
        html.contains("data-mei-view-family=\"world\""),
        "mini-park HTML should expose world stage family"
    );
    assert!(
        html.contains("data-mei-stage-kind=\"world-stage\""),
        "mini-park HTML should expose world stage kind"
    );
    assert!(
        html.contains("data-mei-world-ref=\"park_world\""),
        "mini-park HTML should expose world ref on focus target"
    );
    assert!(
        html.contains("data-mei-group-id=\"park_story_overview\""),
        "mini-park HTML should expose group id on panel or focus target"
    );
    assert!(
        html.contains("data-mei-camera-preset=\"park_overview_orbit\""),
        "mini-park HTML should expose camera preset on panel or focus target"
    );
}

#[test]
fn ws_demo_v2_mini_park_presentation_manifest_emits_world_actions() {
    let presentation = ws_demo_v2_root()
        .join("apps/mini-park/src/presentation/intro.presentation.json");
    if !presentation.is_file() {
        return;
    }
    let manifest = std::fs::read_to_string(&presentation).expect("read intro.presentation.json");
    let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("parse presentation");
    let steps = parsed
        .get("steps")
        .and_then(|value| value.as_array())
        .expect("presentation steps");
    let all_actions = steps
        .iter()
        .flat_map(|step| {
            step.get("actions")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert!(
        all_actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("camera_move")),
        "mini-park presentation should emit camera_move action"
    );
    assert!(
        all_actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("focus_entity")),
        "mini-park presentation should emit focus_entity action"
    );
    assert!(
        all_actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("show_group")),
        "mini-park presentation should emit show_group action"
    );
    assert!(
        all_actions
            .iter()
            .any(|action| action.get("type").and_then(|v| v.as_str()) == Some("enter_world_view")),
        "mini-park presentation should emit enter_world_view action"
    );
    assert!(
        steps.iter().any(|step| step.get("id").and_then(|v| v.as_str()) == Some("enter_lake_pavilion_world")),
        "mini-park presentation should include enter_lake_pavilion_world step"
    );
    assert!(
        steps.iter().any(|step| step.get("id").and_then(|v| v.as_str()) == Some("dual_view_bridge")),
        "mini-park presentation should include dual_view_bridge step"
    );
}

#[test]
fn ws_demo_v2_mini_park_world_plan_from_park_world_mei() {
    let workspace = ensure_mini_park_imported();
    let home_outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park home")
        .expect("mini-park home outcome");
    let worlds = home_outcome
        .world_plan
        .get("worlds")
        .and_then(|v| v.as_object())
        .expect("world_plan worlds");
    let park_world = worlds
        .get("park_world")
        .expect("park_world entry in world_plan");
    assert_eq!(
        park_world.get("id").and_then(|v| v.as_str()),
        Some("park_world")
    );
    let prims = park_world
        .get("primitives")
        .and_then(|v| v.as_array())
        .expect("park_world primitives");
    assert!(
        prims.len() >= 10,
        "park_world should lower park narrative primitives, got {}",
        prims.len()
    );
    let lake_pavilion = prims
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some("lake_pavilion"))
        .expect("lake_pavilion primitive");
    assert!(
        lake_pavilion.get("mapView").is_some() && lake_pavilion.get("worldView").is_some(),
        "lake_pavilion should have dual projection"
    );
    let play_zone = prims
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some("play_zone"))
        .expect("play_zone primitive");
    assert!(
        play_zone.get("mapView").is_some() && play_zone.get("worldView").is_some(),
        "play_zone should have dual projection"
    );
    let view_layers = park_world
        .get("viewLayers")
        .and_then(|v| v.as_array())
        .expect("park_world viewLayers");
    assert!(
        view_layers.iter().any(|layer| {
            layer.get("id").and_then(|v| v.as_str()) == Some("play")
        }),
        "park_world viewLayers should include play layer"
    );
    let projections = home_outcome
        .map_projection
        .get("worlds")
        .and_then(|v| v.as_object())
        .expect("map_projection worlds");
    let park_projection = projections
        .get("park_world")
        .expect("park_world map projection");
    let layers = park_projection
        .get("layers")
        .and_then(|v| v.as_array())
        .expect("park_world projection layers");
    assert!(
        layers.len() >= 5,
        "map_projection should compile park narrative layers"
    );
    let lake_layer = layers
        .iter()
        .find(|l| l.get("id").and_then(|v| v.as_str()) == Some("lake_pavilion"))
        .expect("lake_pavilion projection layer");
    assert_eq!(
        lake_layer
            .get("featureMatch")
            .and_then(|v| v.pointer("/entityId"))
            .and_then(|v| v.as_str()),
        Some("lake_pavilion")
    );
    let home_t1 = home_outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t1"))
        .and_then(|v| v.as_array())
        .expect("home t1 tier");
    assert!(
        home_t1.iter().any(|entry| {
            entry.get("panelId").and_then(|v| v.as_str()) == Some("stage_aperture_frame")
        }),
        "mini-park layer_plan t1 should include stage_aperture_frame for observation window"
    );
}

#[test]
fn ws_demo_v2_mini_park_dual_view_bridge_fixtures_align_with_presentation_map() {
    let workspace = ensure_mini_park_imported();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if !bundle.is_file() {
        return;
    }
    let identity_path = workspace.join("apps/mini-park/prototype/world/park-identity-map.fixture.json");
    let bridge_path = workspace.join("apps/mini-park/prototype/world/park-dual-view-bridge.fixture.json");
    if !identity_path.is_file() || !bridge_path.is_file() {
        return;
    }
    let identity: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&identity_path).expect("read identity fixture"))
            .expect("parse identity fixture");
    let bridge: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&bridge_path).expect("read bridge fixture"))
            .expect("parse bridge fixture");
    let home_outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park home")
        .expect("mini-park home outcome");
    let home_2d_outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home_2d")
        .expect("assemble mini-park home_2d")
        .expect("mini-park home_2d outcome");
    let viewpoints = home_outcome
        .presentation_map
        .get("viewpoints")
        .and_then(|v| v.as_object())
        .expect("home presentation_map viewpoints");
    let overview_id = identity
        .get("overview")
        .and_then(|v| v.get("viewpointId"))
        .and_then(|v| v.as_str())
        .expect("identity overview viewpointId");
    assert!(viewpoints.contains_key(overview_id));
    let points = identity
        .get("points")
        .and_then(|v| v.as_array())
        .expect("identity points");
    for point in points {
        let viewpoint_id = point
            .get("viewpointId")
            .and_then(|v| v.as_str())
            .expect("point viewpointId");
        let entry = viewpoints
            .get(viewpoint_id)
            .unwrap_or_else(|| panic!("missing viewpoint {viewpoint_id} in presentation_map"));
        assert_eq!(
            entry.get("entityId").and_then(|v| v.as_str()),
            point.get("entityId").and_then(|v| v.as_str()),
            "entityId drift for viewpoint {viewpoint_id}"
        );
        assert_eq!(
            entry.get("cameraPreset").and_then(|v| v.as_str()),
            point.get("cameraPreset").and_then(|v| v.as_str()),
            "cameraPreset drift for viewpoint {viewpoint_id}"
        );
    }
    let map_track = bridge
        .get("tracks")
        .and_then(|v| v.get("map_2_5d"))
        .expect("map_2_5d track");
    let svg_track = bridge
        .get("tracks")
        .and_then(|v| v.get("svg_2d"))
        .expect("svg_2d track");
    assert_eq!(
        map_track.get("panelId").and_then(|v| v.as_str()),
        Some("basemap")
    );
    assert_eq!(
        svg_track.get("panelId").and_then(|v| v.as_str()),
        Some("svg_basemap")
    );
    let home_t0 = home_outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t0"))
        .and_then(|v| v.as_array())
        .expect("home t0 tier");
    let home_2d_t0 = home_2d_outcome
        .layer_plan
        .get("tiers")
        .and_then(|v| v.get("t0"))
        .and_then(|v| v.as_array())
        .expect("home_2d t0 tier");
    assert!(
        home_t0.iter().any(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("basemap")),
        "home t0 should include map basemap"
    );
    assert!(
        home_2d_t0
            .iter()
            .any(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("svg_basemap")),
        "home_2d t0 should include svg_basemap"
    );
    let lake = bridge
        .get("objects")
        .and_then(|v| v.get("lake_pavilion"))
        .expect("lake_pavilion bridge object");
    assert_eq!(
        lake.get("projections")
            .and_then(|v| v.get("map_2_5d"))
            .and_then(|v| v.get("extrusionHeight"))
            .and_then(serde_json::Value::as_f64),
        Some(8.6)
    );
    assert_eq!(
        lake.get("projections")
            .and_then(|v| v.get("world_3d"))
            .and_then(|v| v.get("renderFamily"))
            .and_then(|v| v.as_str()),
        Some("extrude_shell")
    );
    let world_track = bridge
        .get("tracks")
        .and_then(|v| v.get("world_3d"))
        .expect("world_3d track");
    assert!(
        world_track.get("prototypeOnly").and_then(|v| v.as_bool()) != Some(true),
        "world_3d track should be runnable"
    );
    assert_eq!(
        world_track.get("panelId").and_then(|v| v.as_str()),
        Some("world_viewport")
    );
    assert_eq!(
        world_track.get("viewFamily").and_then(|v| v.as_str()),
        Some("world")
    );
    let world_entry = viewpoints
        .get("lake_pavilion_world_entry")
        .expect("lake_pavilion_world_entry viewpoint");
    assert_eq!(
        world_entry.get("viewFamily").and_then(|v| v.as_str()),
        Some("world")
    );
    assert_eq!(
        world_entry.get("panelId").and_then(|v| v.as_str()),
        Some("world_viewport")
    );
    assert_eq!(
        world_entry.get("stageKind").and_then(|v| v.as_str()),
        Some("world-stage")
    );
    assert!(
        home_t0
            .iter()
            .any(|entry| entry.get("panelId").and_then(|v| v.as_str()) == Some("world_viewport")),
        "home t0 should include world_viewport"
    );
}

#[test]
fn ws_demo_v2_mini_park_world_stage_contract_compiles() {
    let workspace = ensure_mini_park_imported();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if !bundle.is_file() {
        return;
    }
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park home")
        .expect("mini-park home outcome");
    let contract = outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract");
    let world_panel = contract
        .panels
        .iter()
        .find(|panel| panel.id == "world_viewport")
        .expect("world_viewport panel");
    assert_eq!(
        world_panel.props.get("__mei_view_family").and_then(|v| v.as_str()),
        Some("world")
    );
    assert_eq!(
        world_panel.props.get("__mei_stage_kind").and_then(|v| v.as_str()),
        Some("world-stage")
    );
    let mut world_targets_found = false;
    for node in &world_panel.blocks {
        if let mei_lang_kernel::UiNodeDecl::Block(block) = node {
            if block.use_key == "cockpit.world-stage" {
                world_targets_found = block.props.get("worldTargets").is_some();
            }
        }
    }
    assert!(world_targets_found, "cockpit.world-stage block should declare worldTargets");
}

#[test]
fn ws_demo_v2_mini_park_home_assembles_when_prebuilt() {
    let workspace = ensure_mini_park_imported();
    let bundle = workspace.join("apps/mini-park/build/active/exchange/mini-park.meibundle");
    if !bundle.is_file() {
        return;
    }
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-park", "home")
        .expect("assemble mini-park")
        .expect("mini-park home outcome");
    assert_eq!(
        outcome.compiled.app_id, "mini-park",
        "mini-park home should assemble when bundle exists"
    );
}
