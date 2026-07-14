//! Home scene shape parity: v2 assemble vs v1 kernel compile (ws-data-demo reference).

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::{compile_app_from_root_with_options, CompileOptions, UiNodeDecl, UiTreeNode};

static V2_INIT: Once = Once::new();

fn ws_data_demo_v1() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-data-demo")
        .canonicalize()
        .expect("ws-data-demo")
}

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn skip_if_data_demo_missing() -> Option<PathBuf> {
    let workspace = ws_demo_v2();
    if !workspace.join("apps/data-demo").is_dir() {
        return None;
    }
    Some(workspace)
}

fn ensure_v2_imported() -> Option<PathBuf> {
    let workspace = skip_if_data_demo_missing()?;
    V2_INIT.call_once(|| {
        let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
        if !bundle.is_file() {
            return;
        }
        let ctx = HostContext::new(workspace.clone(), "data-demo");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import");
    });
    let bundle = workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle");
    if !bundle.is_file() {
        return None;
    }
    Some(workspace)
}

fn collect_use_keys(panels: &[UiNodeDecl]) -> Vec<String> {
    let mut keys = Vec::new();
    fn walk(panel: &UiNodeDecl, out: &mut Vec<String>) {
        for node in &panel.blocks {
            match node {
                UiTreeNode::Block(block) => out.push(block.use_key.clone()),
                UiTreeNode::Panel(nested) => walk(nested, out),
                UiTreeNode::PanelRefEmbed(_) => {}
            }
        }
    }
    for panel in panels {
        walk(panel, &mut keys);
    }
    keys
}

fn panel_titles(panels: &[UiNodeDecl]) -> Vec<String> {
    let mut titles = Vec::new();
    fn walk(panel: &UiNodeDecl, out: &mut Vec<String>) {
        if let Some(title) = panel.title.as_deref().filter(|t| !t.is_empty()) {
            out.push(title.to_string());
        }
        for node in &panel.blocks {
            if let UiTreeNode::Panel(nested) = node {
                walk(nested, out);
            }
        }
    }
    for panel in panels {
        walk(panel, &mut titles);
    }
    titles
}

#[test]
fn home_v2_matches_v1_component_shape() {
    let Some(_) = ensure_v2_imported() else { return; };
    let v2 = assemble_scope_from_registry(ws_demo_v2().as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home");
    let v2_contract = v2.compiled.scene_contract.as_ref().expect("contract");

    let v1_root = ws_data_demo_v1();
    let v1_app = v1_root.join("apps/data-demo");
    let v1 = compile_app_from_root_with_options(
        v1_root.as_path(),
        v1_app.as_path(),
        CompileOptions {
            scene: Some("home".to_string()),
            ..Default::default()
        },
    )
    .expect("v1 compile home");
    let v1_contract = v1.scene_contract.as_ref().expect("v1 contract");

    assert_eq!(
        v1_contract.panels.len(),
        v2_contract.panels.len(),
        "top-level panel count"
    );
    let v1_keys = collect_use_keys(&v1_contract.panels);
    let v2_keys = collect_use_keys(&v2_contract.panels);
    for key in ["cockpit.header-brand", "cockpit.data-table", "mei.text"] {
        assert!(v1_keys.iter().any(|k| k == key), "v1 home missing {key}");
        assert!(
            v2_keys.iter().any(|k| k == key),
            "v2 home missing {key}; got {v2_keys:?}"
        );
    }
    assert!(
        v2_keys.iter().filter(|k| *k == "mei.text").count() >= 10,
        "v2 home should expand metric_card to mei.text slots, got {}",
        v2_keys.iter().filter(|k| *k == "mei.text").count()
    );

    let v2_titles = panel_titles(&v2_contract.panels);
    for title in ["实时预警", "监督预警", "问题办理", "监督成效", "典型案例"] {
        assert!(
            v2_titles.iter().any(|t| t.contains(title)),
            "v2 missing panel title `{title}`; got {v2_titles:?}"
        );
    }
}

fn walk_panels<'a>(panels: &'a [UiNodeDecl], f: &mut dyn FnMut(&'a UiNodeDecl)) {
    for panel in panels {
        f(panel);
        for node in &panel.blocks {
            if let UiTreeNode::Panel(nested) = node {
                walk_panels(std::slice::from_ref(nested), f);
            }
        }
    }
}

#[test]
fn home_v2_supervision_metric_card_inherits_solid_stack_shell() {
    let Some(_) = ensure_v2_imported() else { return; };
    let v2 = assemble_scope_from_registry(ws_demo_v2().as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home");
    let contract = v2.compiled.scene_contract.as_ref().expect("contract");
    let mut card = None;
    walk_panels(&contract.panels, &mut |panel| {
        if panel.id == "supervision_items_card" {
            card = Some(panel);
        }
    });
    let card = card.expect("supervision_items_card");
    assert_eq!(
        v2.compiled
            .scene_contract
            .as_ref()
            .and_then(|c| c.scene.theme.as_deref()),
        Some("cockpit"),
        "home scene should use cockpit theme for metric/table chrome"
    );
    assert!(
        card.props
            .get("__mei_metric_card")
            .and_then(|v| v.as_bool())
            == Some(true),
        "solid_stack card props: {:?}",
        card.props
    );
    assert!(
        card.props
            .get("border")
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.contains("98,190,235")),
        "expected cockpit border on supervision card, got {:?}",
        card.props.get("border")
    );
}

#[test]
fn home_v2_resolves_metric_card_link_ref_popup() {
    let Some(_) = ensure_v2_imported() else { return; };
    let v2 = assemble_scope_from_registry(ws_demo_v2().as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home");
    let contract = v2.compiled.scene_contract.as_ref().expect("contract");
    let mut popup = None;
    fn walk(panel: &UiNodeDecl, target: &str, out: &mut Option<serde_json::Value>) {
        if panel.id == target {
            for node in &panel.blocks {
                if let UiTreeNode::Block(block) = node {
                    if block.props.get("metric_role").and_then(|v| v.as_str()) == Some("value") {
                        if let Some(p) = block.props.get("popup") {
                            *out = Some(p.clone());
                        }
                    }
                }
            }
        }
        for node in &panel.blocks {
            if let UiTreeNode::Panel(nested) = node {
                walk(nested, target, out);
            }
        }
    }
    walk(&contract.panels[0], "supervision_items_card", &mut popup);
    // search all panels if not found at top
    if popup.is_none() {
        fn walk_all(panels: &[UiNodeDecl], out: &mut Option<serde_json::Value>) {
            for panel in panels {
                for node in &panel.blocks {
                    if let UiTreeNode::Block(block) = node {
                        if block.props.get("metric_role").and_then(|v| v.as_str()) == Some("value")
                            && panel.id == "supervision_items_card"
                        {
                            *out = block.props.get("popup").cloned();
                        }
                    }
                    if let UiTreeNode::Panel(nested) = node {
                        walk_all(std::slice::from_ref(nested), out);
                    }
                }
            }
        }
        walk_all(&contract.panels, &mut popup);
    }
    let popup = popup.expect("value slot popup");
    eprintln!("popup={}", serde_json::to_string_pretty(&popup).unwrap());
    assert!(
        popup.get("__ref").and_then(|v| v.as_str()) != Some("link_ref"),
        "popup should be resolved, got {popup}"
    );
    assert!(
        popup.get("scene_id").and_then(|v| v.as_str()) == Some("supervision_items_analytics_board"),
        "unexpected popup {popup}"
    );
}

#[test]
fn home_v2_analytics_board_assemblies_include_projection_slots() {
    let Some(_) = ensure_v2_imported() else { return; };
    let v2 = assemble_scope_from_registry(ws_demo_v2().as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home");
    let assemblies = &v2.compiled.scene_projection_assembly_by_id;
    let sample_ids = [
        "supervision_items_analytics_board",
        "warnings_analytics_board",
        "effect_transfer_clue_analytics_board",
        "issue_pending_analytics_board",
        "typical_cases_detail_board",
    ];
    for scene_id in sample_ids {
        let assembly = assemblies
            .get(scene_id)
            .unwrap_or_else(|| panic!("missing assembly for {scene_id}"));
        let slots = assembly
            .get("projection_slots")
            .and_then(|value| value.as_array())
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                panic!(
                    "expected projection_slots for {scene_id}, assembly keys: {:?}",
                    assembly
                        .as_object()
                        .map(|map| map.keys().collect::<Vec<_>>())
                )
            });
        assert!(
            assembly
                .get("shell_contract")
                .and_then(|shell| shell.get("layout_mode"))
                .and_then(|value| value.as_str())
                .is_some(),
            "shell_contract.layout_mode required for {scene_id}"
        );
        assert!(
            !slots.is_empty(),
            "projection_slots should not be empty for {scene_id}"
        );
    }
}
