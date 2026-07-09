use std::path::PathBuf;
use std::sync::Once;
use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, clear_assemble_cache_for_app, import_bundle, ImportOptions,
};
use mei_lang_kernel::build_ui_layout_index;

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(&ctx, &ImportOptions { bundle_path: Some(bundle) }).expect("import");
        clear_assemble_cache_for_app("pretty-panels");
    });
    workspace
}

fn find_panel_by_id<'a>(panels: &'a [mei_lang_kernel::PanelDecl], id: &str) -> Option<&'a mei_lang_kernel::PanelDecl> {
    for panel in panels {
        if panel.id == id || panel.id.ends_with(&format!("/{id}")) {
            return Some(panel);
        }
        for node in &panel.blocks {
            if let mei_lang_kernel::UiNodeDecl::Panel(child) = node {
                if let Some(found) = find_panel_by_id(std::slice::from_ref(child), id) {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[test]
fn pretty_panels_map_stage_resolves_maplibre_in_region_tree() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let contract = outcome.compiled.scene_contract.as_ref().unwrap();
    let map_stage = find_panel_by_id(&contract.panels, "map_stage").expect("map_stage region panel");
    assert_eq!(
        map_stage.props.get("__mei_view_family").and_then(|v| v.as_str()),
        Some("map"),
        "map_stage props: {:?}",
        map_stage.props
    );
    fn uses_maplibre(panel: &mei_lang_kernel::PanelDecl) -> bool {
        panel.blocks.iter().any(|node| match node {
            mei_lang_kernel::UiNodeDecl::Block(block) => block.use_key == "map.maplibre",
            mei_lang_kernel::UiNodeDecl::Panel(child) => uses_maplibre(child),
            mei_lang_kernel::UiNodeDecl::PanelRefEmbed(_) => false,
        })
    }
    assert!(
        uses_maplibre(map_stage),
        "map_stage should nest map.maplibre block, blocks={}",
        map_stage.blocks.len()
    );
    let center_rail = find_panel_by_id(&contract.panels, "center_rail").expect("center_rail region");
    let map_viewport_section = center_rail
        .blocks
        .iter()
        .find_map(|block| match block {
            mei_lang_kernel::UiNodeDecl::Panel(section) if section.id == "map_viewport" => {
                Some(section)
            }
            _ => None,
        })
        .expect("map_viewport section under center_rail");
    assert!(
        map_viewport_section.title.is_none(),
        "map_viewport should use bare shell without section title, got {:?}",
        map_viewport_section.title
    );
    assert!(
        find_panel_by_id(&contract.panels, "map-viewport").is_some(),
        "map-viewport content should nest under center_rail"
    );
    let ui = build_ui_layout_index(&outcome.compiled);
    assert!(
        ui.index
            .nodes
            .keys()
            .any(|k| k.contains("map-tools-slot") || k.contains("/tools")),
        "ui_layout_index should include map_viewport operation chrome under center_rail"
    );
    fn panel_has_content_role_child(panel: &mei_lang_kernel::PanelDecl) -> bool {
        panel.blocks.iter().any(|node| match node {
            mei_lang_kernel::UiNodeDecl::Panel(child) => {
                child.props.get("__mei_ui_role").and_then(|v| v.as_str()) == Some("content")
                    || panel_has_content_role_child(child)
            }
            mei_lang_kernel::UiNodeDecl::Block(_) | mei_lang_kernel::UiNodeDecl::PanelRefEmbed(_) => {
                false
            }
        })
    }
    assert!(
        !panel_has_content_role_child(map_stage),
        "map_stage should not keep content-role wrapper sections"
    );
    fn panel_has_section_role(panel: &mei_lang_kernel::PanelDecl) -> bool {
        matches!(
            panel.props.get("__mei_ui_role").and_then(|v| v.as_str()),
            Some("section") | Some("stage")
        )
    }
    fn find_section_role_panel(panel: &mei_lang_kernel::PanelDecl) -> Option<&mei_lang_kernel::PanelDecl> {
        if panel_has_section_role(panel) {
            return Some(panel);
        }
        panel.blocks.iter().find_map(|node| match node {
            mei_lang_kernel::UiNodeDecl::Panel(child) => find_section_role_panel(child),
            _ => None,
        })
    }
    let section = find_section_role_panel(map_stage).expect("map_stage should nest a section/stage panel");
    assert!(panel_has_section_role(section), "map section props: {:?}", section.props);
}

#[test]
fn pretty_panels_ui_structure_includes_header_section() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let header = find_panel_by_id(
        &outcome
            .compiled
            .scene_contract
            .as_ref()
            .unwrap()
            .panels,
        "home_header",
    )
    .expect("home_header region panel");
    let header_section = header
        .blocks
        .iter()
        .find_map(|block| match block {
            mei_lang_kernel::UiNodeDecl::Panel(section) if section.id == "header" => Some(section),
            _ => None,
        })
        .expect("header section under home_header region");
    assert!(
        !header_section.blocks.is_empty(),
        "header section should contain screen_header blocks"
    );

    let ui = build_ui_layout_index(&outcome.compiled);
    assert!(
        ui.index.nodes.keys().any(|k| {
            k.contains("home_header") || k.contains("/t1/header") || k.contains("/header/body")
        }),
        "ui_layout_index should include header section path: {:?}",
        ui.index.nodes.keys().collect::<Vec<_>>()
    );
}

#[test]
fn pretty_panels_ui_structure_includes_left_rail_sections() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let left_rail = find_panel_by_id(
        &outcome.compiled.scene_contract.as_ref().unwrap().panels,
        "left_rail",
    )
    .expect("left_rail");
    eprintln!("left_rail blocks: {}", left_rail.blocks.len());
    for b in &left_rail.blocks {
        if let mei_lang_kernel::UiNodeDecl::Panel(p) = b {
            eprintln!("  section panel id={} title={:?} blocks={}", p.id, p.title, p.blocks.len());
        }
    }
    let ui = build_ui_layout_index(&outcome.compiled);
    let enforcement = ui.index.nodes.keys().find(|k| k.contains("enforcement"));
    eprintln!("ui nodes with enforcement: {:?}", enforcement);
    eprintln!("ui index node count: {}", ui.index.nodes.len());
    assert!(
        ui.index.nodes.keys().any(|k| {
            k.contains("left_rail/enforcement") || k.contains("t1/left_rail/enforcement")
        }),
        "missing enforcement in ui index: {:?}",
        ui.index.nodes.keys().collect::<Vec<_>>()
    );

    let enforcement_panel = find_panel_by_id(
        &outcome.compiled.scene_contract.as_ref().unwrap().panels,
        "enforcement",
    )
    .expect("enforcement section panel");
    assert!(
        !enforcement_panel.blocks.is_empty(),
        "enforcement section should contain lowered metric blocks"
    );
}

#[test]
fn pretty_panels_penalty_section_surfaces_contract_level_charts() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let ui = build_ui_layout_index(&outcome.compiled);
    let penalty_scopes: Vec<_> = ui
        .index
        .nodes
        .values()
        .filter(|node| {
            node.preview_scope.contains("left_rail/penalty")
                || node.preview_scope.contains("t1/left_rail/penalty")
        })
        .map(|node| {
            format!(
                "{} {:?} {}",
                node.preview_scope,
                node.role,
                node.label
            )
        })
        .collect();
    eprintln!("penalty ui scopes:\n{}", penalty_scopes.join("\n"));
    assert!(
        ui.index.nodes.values().any(|node| {
            node.preview_scope.contains("party_bars")
                && node.role == mei_lang_kernel::UiScopeRole::Slot
        }),
        "penalty party_bars grid slot should surface in ui index"
    );
    assert!(
        ui.index.nodes.values().any(|node| {
            node.preview_scope.contains("left_rail/penalty")
                && node.role == mei_lang_kernel::UiScopeRole::Content
                && (node.label.contains("罚没") || node.label.contains("分组柱图") || node.label.contains("排名图") || node.label.contains("高频"))
        }),
        "penalty contract-level charts should surface in ui index"
    );
}

#[test]
fn pretty_panels_assemble_accepts_legacy_assembly_scene_id() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "assembly")
        .expect("assemble")
        .expect("home via assembly alias");
    assert_eq!(
        outcome.compiled.active_scene.as_deref(),
        Some("home"),
        "legacy scene id assembly should resolve to home"
    );
}
