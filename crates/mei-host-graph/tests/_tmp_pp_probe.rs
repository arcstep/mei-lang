#[test]
fn probe_pretty_panels_issue() {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2").canonicalize().unwrap();
    let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
    let ctx = mei_host_core::HostContext::new(workspace.clone(), "pretty-panels");
    mei_host_graph::import_bundle(&ctx, &mei_host_graph::ImportOptions { bundle_path: Some(bundle) }).unwrap();
    mei_host_graph::clear_assemble_cache_for_app("pretty-panels");
    let outcome = mei_host_graph::assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home").unwrap().unwrap();
    let panels = &outcome.compiled.scene_contract.as_ref().unwrap().panels;
    fn walk(panels: &[mei_lang_kernel::UiNodeDecl], path: &str, out: &mut Vec<String>) {
        for p in panels {
            let here = format!("{path}/{}", p.id);
            if here.contains("issue") || here.contains("pending") || here.contains("summary") {
                let bg = p.props.get("background").map(|v| serde_json::to_string(v).unwrap_or_default()).unwrap_or_default();
                out.push(format!("{here} blocks={} bg={}", p.blocks.len(), bg.chars().take(80).collect::<String>()));
            }
            for node in &p.blocks {
                if let mei_lang_kernel::UiTreeNode::Panel(child) = node {
                    walk(std::slice::from_ref(child), &here, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(panels, "", &mut out);
    for line in &out { eprintln!("{line}"); }
    assert!(!out.is_empty(), "expected issue panels");
}
