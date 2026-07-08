use std::path::PathBuf;

use mei_host_graph::{
    assemble_scope_from_registry, build_eval_slot_group_document, build_structure_full_document,
    collect_slot_groups,
};
use mei_lang_kernel::DataMode;

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

#[test]
fn pretty_panels_enforcement_units_slot_exports_shell_and_mounts() {
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let structure = build_structure_full_document(&outcome.compiled, "test");
    let enforcement_nodes: Vec<_> = structure
        .nodes
        .iter()
        .filter(|node| node.preview_scope.contains("enforcement_units"))
        .map(|node| {
            (
                node.preview_scope.clone(),
                node.ui_role.clone(),
                node.label.clone(),
                node.content_kind.clone(),
            )
        })
        .collect();
    eprintln!("enforcement_units structure nodes: {enforcement_nodes:#?}");

    let docs: Vec<_> = collect_slot_groups(&structure)
        .into_iter()
        .map(|group| {
            build_eval_slot_group_document(
                &outcome.compiled,
                &structure,
                group.as_str(),
                DataMode::Eval,
                Some(ws_demo_v2().as_path()),
            )
        })
        .collect();
    let mut eval_hits = Vec::new();
    for doc in &docs {
        for (scope, slot) in &doc.slots {
            if scope.contains("enforcement_units") {
                eval_hits.push((
                    scope.clone(),
                    slot.get("panel_shell").is_some(),
                    slot.get("component_mounts")
                        .and_then(|value| value.as_array())
                        .map(|mounts| mounts.len())
                        .unwrap_or(0),
                ));
            }
        }
    }
    eprintln!("enforcement_units eval slots: {eval_hits:#?}");

    let shell_scope = eval_hits
        .iter()
        .find(|(scope, has_shell, mounts)| {
            scope.contains("enforcement_units_card") && (*has_shell || *mounts > 0)
        });
    assert!(
        shell_scope.is_some(),
        "expected metric slot shell/mounts on enforcement_units_card scope, got {eval_hits:#?}"
    );
}
