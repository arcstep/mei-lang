use std::path::PathBuf;
use std::sync::Once;
use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
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
    let map_stage = contract
        .panels
        .iter()
        .find(|p| p.id == "map_stage")
        .expect("map_stage region panel");
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
    let viewport_frame = find_panel_by_id(&contract.panels, "viewport_frame")
        .expect("viewport_frame nested under center_rail map viewport");
    assert_eq!(
        viewport_frame.props.get("variant").and_then(|v| v.as_str()),
        Some("container")
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
    let section = match &map_stage.blocks[0] {
        mei_lang_kernel::UiNodeDecl::Panel(panel) => panel,
        other => panic!("expected section panel, got {other:?}"),
    };
    assert!(
        matches!(
            section.props.get("__mei_ui_role").and_then(|v| v.as_str()),
            Some("section") | Some("stage")
        ),
        "map section props: {:?}",
        section.props
    );
}

#[test]
fn pretty_panels_ui_structure_includes_header_section() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let header = outcome
        .compiled
        .scene_contract
        .as_ref()
        .unwrap()
        .panels
        .iter()
        .find(|p| p.id == "home_header")
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
            k.contains("home_header") && (k.contains("/header") || k.contains("header/"))
        }),
        "ui_layout_index should include home_header section path: {:?}",
        ui.index.nodes.keys().collect::<Vec<_>>()
    );
}

#[test]
fn pretty_panels_ui_structure_includes_left_rail_sections() {
    let outcome = assemble_scope_from_registry(ensure_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home");
    let left_rail = outcome.compiled.scene_contract.as_ref().unwrap().panels.iter().find(|p| p.id == "left_rail").expect("left_rail");
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
    assert!(ui.index.nodes.keys().any(|k| k.contains("left_rail/enforcement")), "missing enforcement in ui index: {:?}", ui.index.nodes.keys().collect::<Vec<_>>());

    let enforcement_panel = outcome
        .compiled
        .scene_contract
        .as_ref()
        .unwrap()
        .panels
        .iter()
        .find(|p| p.id == "left_rail")
        .and_then(|rail| {
            rail.blocks.iter().find_map(|block| match block {
                mei_lang_kernel::UiNodeDecl::Panel(section) if section.id == "enforcement" => {
                    Some(section)
                }
                _ => None,
            })
        })
        .expect("enforcement section panel");
    assert!(
        !enforcement_panel.blocks.is_empty(),
        "enforcement section should contain lowered metric blocks"
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
