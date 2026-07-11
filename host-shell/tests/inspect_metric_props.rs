use mei_host_graph::assemble_scope_from_registry;
use mei_lang_kernel::{UiNodeDecl, UiTreeNode};

#[test]
fn home_inspection_total_card_has_metric_source() {
    let workspace =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-demo-v2");
    if !workspace.join("apps/data-demo/app.config.json").is_file() {
        return;
    }
    let outcome = assemble_scope_from_registry(workspace.as_path(), "data-demo", "home")
        .expect("assemble")
        .expect("home");
    let contract = outcome.compiled.scene_contract.as_ref().expect("contract");
    let mut paths = Vec::new();
    for panel in &contract.panels {
        collect_paths(panel, "", &mut paths);
    }
    eprintln!(
        "panel paths containing inspection: {:?}",
        paths
            .iter()
            .filter(|p| p.contains("inspection"))
            .collect::<Vec<_>>()
    );
    let mut found_source = None;
    for panel in &contract.panels {
        walk_panel(panel, &mut found_source);
    }
    assert!(
        found_source.is_some(),
        "inspection_total_card block not found"
    );
    let source = found_source.unwrap();
    eprintln!("source = {}", source);
    assert!(
        source.contains("inspections_total_count"),
        "missing metric id in source"
    );
    assert!(
        !source.contains("__var"),
        "bundle constant should be resolved in lowered block, got {source}"
    );
}

fn collect_paths(panel: &UiNodeDecl, prefix: &str, out: &mut Vec<String>) {
    let path = if prefix.is_empty() {
        panel.id.clone()
    } else {
        format!("{prefix}/{}", panel.id)
    };
    out.push(path.clone());
    for child in &panel.blocks {
        if let UiTreeNode::Panel(nested) = child {
            collect_paths(nested, path.as_str(), out);
        }
    }
}

fn walk_panel(panel: &UiNodeDecl, out: &mut Option<String>) {
    if panel.id == "inspection_total_card" {
        eprintln!("inspection_total_card blocks: {}", panel.blocks.len());
        for child in &panel.blocks {
            match child {
                UiTreeNode::Block(block) => {
                    eprintln!(
                        "  block use_key={} props_keys={:?}",
                        block.use_key,
                        block
                            .props
                            .as_object()
                            .map(|m| m.keys().collect::<Vec<_>>())
                    );
                    if block.use_key == "mei.text" || block.use_key == "mei-text" {
                        let content = block.props.get("content");
                        eprintln!(
                            "  content={}",
                            serde_json::to_string(&content).unwrap_or_default()
                        );
                        if let Some(content) = content {
                            *out = Some(serde_json::to_string(content).unwrap_or_default());
                        }
                    }
                }
                UiTreeNode::Panel(nested) => eprintln!("  nested panel {}", nested.id),
                UiTreeNode::PanelRefEmbed(_) => eprintln!("  panel ref embed"),
            }
        }
    }
    for child in &panel.blocks {
        walk(child, out);
    }
}

fn walk(node: &UiTreeNode, out: &mut Option<String>) {
    match node {
        UiTreeNode::Panel(panel) => walk_panel(panel, out),
        UiTreeNode::Block(_) | UiTreeNode::PanelRefEmbed(_) => {}
    }
}
