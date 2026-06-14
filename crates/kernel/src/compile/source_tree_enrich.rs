use std::path::Path;

use crate::model::WorkspaceNode;

use super::load_external::load_scene_decls_from_file;

pub(super) fn enrich_source_tree_with_scene_exports(
    app_root: &Path,
    nodes: &mut [WorkspaceNode],
) {
    for node in nodes.iter_mut() {
        if node.kind == "dir" {
            enrich_source_tree_with_scene_exports(app_root, &mut node.children);
            continue;
        }
        if node.kind != "file" || !node.path.ends_with(".mei") {
            continue;
        }
        let Ok(scenes) = load_scene_decls_from_file(app_root, node.path.as_str()) else {
            continue;
        };
        if scenes.len() <= 1 {
            continue;
        }
        node.children = scenes
            .into_iter()
            .map(|scene| WorkspaceNode {
                name: scene.id.clone(),
                path: node.path.clone(),
                kind: "scene_export".to_string(),
                mei_kind: Some("scene".to_string()),
                scene_export_id: Some(scene.id),
                world_dataset_id: None,
                world_metric_id: None,
                explain_block_id: None,
                semantic_label: None,
                children: Vec::new(),
            })
            .collect();
    }
}
