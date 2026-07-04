use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::{PanelDecl, UiNodeDecl};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_pretty_panels_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        assert!(
            bundle.is_file(),
            "run `mei-compiler compile --workspace ws-demo-v2 --app pretty-panels` first"
        );
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

fn find_panel<'a>(panel: &'a PanelDecl, id: &str) -> Option<&'a PanelDecl> {
    if panel.id == id {
        return Some(panel);
    }
    for block in &panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            if let Some(found) = find_panel(nested, id) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn pretty_panels_enforcement_section_carries_body_padding() {
    let outcome = assemble_scope_from_registry(ensure_pretty_panels_imported().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let panels = &outcome
        .compiled
        .scene_contract
        .as_ref()
        .expect("scene contract")
        .panels;
    let left_rail = panels
        .iter()
        .find(|panel| panel.id == "left_rail")
        .expect("left_rail panel");
    let enforcement = find_panel(left_rail, "enforcement").expect("enforcement section");
    assert_eq!(
        enforcement
            .body_props
            .get("padding")
            .and_then(|v| v.as_str()),
        Some("8px 4px 2px 4px"),
        "body_props: {:?}",
        enforcement.body_props,
    );
}
