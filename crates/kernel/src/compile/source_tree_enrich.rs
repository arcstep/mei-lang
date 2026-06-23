use std::path::Path;

use serde_json::Value;

use crate::model::{SceneDecl, WorkspaceNode};

use super::load_external::load_scene_decls_from_file;

fn scene_display_label(scene: &SceneDecl) -> String {
    if let Some(title) = scene_example_title(&scene.examples) {
        return title;
    }
    if let Some(summary) = scene
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return summary.to_string();
    }
    scene.id.clone()
}

fn scene_example_title(examples: &Value) -> Option<String> {
    let items = examples.as_array()?;
    for item in items {
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())?;
        return Some(title.to_string());
    }
    None
}

pub(crate) fn enrich_source_tree_with_scene_exports(app_root: &Path, nodes: &mut [WorkspaceNode]) {
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
        let is_board_capsule = node.path.ends_with(".board.mei");
        // 普通 scene 文件仅多 export 时展开；board capsule 即使单 export 也要进语义 DAG / Boards 树。
        if scenes.is_empty() || (!is_board_capsule && scenes.len() <= 1) {
            continue;
        }
        node.children = scenes
            .into_iter()
            .map(|scene| WorkspaceNode {
                name: scene_display_label(&scene),
                path: node.path.clone(),
                kind: "scene_export".to_string(),
                mei_kind: Some(if is_board_capsule {
                    "board".to_string()
                } else {
                    "scene".to_string()
                }),
                scene_export_id: Some(scene.id.clone()),
                world_dataset_id: None,
                world_metric_id: None,
                explain_block_id: None,
                semantic_label: Some(scene.id.clone()),
                children: Vec::new(),
            })
            .collect();
    }
}
