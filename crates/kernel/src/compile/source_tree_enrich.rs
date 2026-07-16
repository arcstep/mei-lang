use std::path::Path;

use mei_syntax::v2::{parse_v2_source, CallArgs, V2Expr, V2Item};
use serde_json::Value;

use crate::mei_config::is_plane_structure_mei_path;
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

fn call_arg_string(args: &CallArgs, key: &str) -> Option<String> {
    let expr = args.keywords.iter().find(|(name, _)| name == key)?.1.clone();
    match expr {
        V2Expr::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

fn page_instance_exports_from_mei(source: &str) -> Vec<(String, String)> {
    let Ok(file) = parse_v2_source(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in file.items {
        let V2Item::TopLevel { name, args } = item else {
            continue;
        };
        if name != "page_instance" {
            continue;
        }
        let Some(key) = call_arg_string(&args, "key") else {
            continue;
        };
        let scene = call_arg_string(&args, "scene").unwrap_or_else(|| key.clone());
        out.push((key, scene));
    }
    out
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

        // T2 plane capsules: expose page_instance exports for Boards / IDE index.
        if is_plane_structure_mei_path(node.path.as_str()) {
            let abs = app_root.join(node.path.as_str());
            if let Ok(source) = std::fs::read_to_string(&abs) {
                let exports = page_instance_exports_from_mei(&source);
                if !exports.is_empty() {
                    node.children = exports
                        .into_iter()
                        .map(|(key, scene)| WorkspaceNode {
                            name: scene.clone(),
                            path: node.path.clone(),
                            kind: "page_instance_export".to_string(),
                            mei_kind: Some("t2_page".to_string()),
                            scene_export_id: Some(scene),
                            world_dataset_id: None,
                            world_metric_id: None,
                            explain_block_id: None,
                            semantic_label: Some(key),
                            children: Vec::new(),
                        })
                        .collect();
                    continue;
                }
            }
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
