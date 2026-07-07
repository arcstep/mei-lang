//! Build overview layout / theme.layout diff via enriched assemble.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::{
    build_ui_layout_index, load_mei_config_for_app, PanelDecl, UiNodeDecl, UiScopeRole,
};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_pretty_panels_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import pretty-panels bundle");
    });
    workspace
}

fn find_panel_by_id<'a>(panel: &'a PanelDecl, id: &str) -> Option<&'a PanelDecl> {
    if panel.id == id {
        return Some(panel);
    }
    for block in &panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            if let Some(found) = find_panel_by_id(nested, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_panel_in_tree<'a>(panels: &'a [PanelDecl], id: &str) -> Option<&'a PanelDecl> {
    panels
        .iter()
        .find_map(|panel| find_panel_by_id(panel, id))
}

#[test]
fn pretty_panels_enriched_assemble_has_theme_layout_padding() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let compiled = outcome.compiled;
    assert!(
        !compiled.ui_layout_index.nodes.is_empty(),
        "assemble should populate ui_layout_index"
    );
    let panels = &compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let enforcement = find_panel_in_tree(panels, "enforcement").expect("enforcement section");
    assert_eq!(
        enforcement
            .props
            .get("__mei_padding_profile")
            .and_then(|v| v.as_str()),
        Some("dense_strip_100"),
        "theme.layout paddingProfile should merge into enforcement section"
    );
    assert_eq!(
        enforcement
            .body_props
            .get("padding")
            .and_then(|v| v.as_str()),
        Some("8px 4px 2px 4px")
    );
    let config = load_mei_config_for_app(
        std::path::Path::new(compiled.app_root.as_str()),
        Some(workspace.as_path()),
    );
    assert!(
        config.ops.layout_tuning.is_none(),
        "pretty-panels should not use layoutTuning after migration"
    );
    let theme_layout = config
        .ops
        .themes
        .get("cockpit")
        .and_then(|theme| theme.get("layout"))
        .and_then(|layout| layout.get("home/T1/left_rail/enforcement"));
    assert_eq!(
        theme_layout
            .and_then(|v| v.get("paddingProfile"))
            .and_then(|v| v.as_str()),
        Some("dense_strip_100")
    );
    let ui = build_ui_layout_index(&compiled);
    assert!(
        ui.index.nodes.values().any(|node| {
            node.role == UiScopeRole::Section
                && (node.preview_scope.ends_with("/left_rail")
                    || node.preview_scope == "left_rail/enforcement")
        }),
        "rebuilt index should include left_rail section scope"
    );
}
