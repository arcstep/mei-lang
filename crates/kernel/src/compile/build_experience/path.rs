use super::{
    block_label, find_block_in_panel, find_panel_by_path, panel_label, projection_label,
    scene_label, split_block_key, split_file_symbol, split_panel_key, split_projection_key,
    split_world_explain_key,
};

use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp, ExperienceNodeManifest};

/// Human-readable breadcrumb segments for build overview / agent export.
pub fn build_experience_path(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    if let Some(manifest) = ExperienceNodeManifest::lookup(compiled, node) {
        if !manifest.experience_path.is_empty() {
            return manifest.experience_path.clone();
        }
    }
    build_experience_path_runtime(compiled, node)
}

fn build_experience_path_runtime(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    match node.kind {
        BuildNodeKind::Route | BuildNodeKind::Scene => scene_label(compiled, &node.key),
        BuildNodeKind::Projection => {
            let (scene_id, projection_id) = split_projection_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            path.push(projection_label(projection_id.as_str()));
            path
        }
        BuildNodeKind::ScenePanel => {
            let (scene_id, panel_path) = split_panel_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) {
                path.push(panel_label(&panel));
            } else {
                path.push(panel_path);
            }
            path
        }
        BuildNodeKind::SceneBlock => {
            let (scene_id, panel_path, block_id) = split_block_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) {
                path.push(panel_label(&panel));
                if let Some(block) = find_block_in_panel(&panel, block_id.as_str()) {
                    path.push(block_label(&block));
                } else {
                    path.push(block_id);
                }
            } else {
                path.push(panel_path);
                path.push(block_id);
            }
            path
        }
        BuildNodeKind::UiScope => compiled
            .ui_layout_index
            .lookup(node)
            .map(|entry| {
                let mut path = scene_label(compiled, entry.scene_id.as_deref().unwrap_or(""));
                path.extend(entry.scope_path.iter().skip(1).cloned());
                path
            })
            .unwrap_or_else(|| vec![node.encode()]),
        BuildNodeKind::WorldFile => vec!["Backing · World".to_string(), node.key.clone()],
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric => {
            let (file, symbol) = split_file_symbol(&node.key);
            vec!["Backing · World".to_string(), file, symbol]
        }
        BuildNodeKind::WorldExplain => {
            let (file, metric, explain) = split_world_explain_key(&node.key);
            vec!["Backing · World".to_string(), file, metric, explain]
        }
        BuildNodeKind::Dataset => vec!["Backing · Datasets".to_string(), node.key.clone()],
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            if let Some(entry) = compiled.build_t2_page_index.lookup(node) {
                vec![
                    "Board".to_string(),
                    entry.label.clone(),
                    entry.scene_id.clone(),
                ]
            } else {
                vec!["Board".to_string(), node.key.clone()]
            }
        }
        BuildNodeKind::Component => {
            if let Some(entry) = compiled.build_template_index.lookup(node.key.as_str()) {
                vec![
                    "Component".to_string(),
                    entry.template_key.clone(),
                    entry.template_file.clone(),
                ]
            } else {
                vec!["Component".to_string(), node.key.clone()]
            }
        }
        BuildNodeKind::Template => {
            if let Some(entry) = compiled.build_template_index.lookup(node.key.as_str()) {
                let mut rows = vec![
                    "Template".to_string(),
                    entry.template_key.clone(),
                    entry.template_file.clone(),
                ];
                if let Some(anchor) =
                    crate::compile::build_template_index::template_primary_consumer(
                        compiled,
                        entry.template_key.as_str(),
                    )
                {
                    rows.push(format!(
                        "→ {} / {} / {}",
                        anchor.scene_id, anchor.panel_path, anchor.label
                    ));
                }
                rows
            } else {
                vec!["Template".to_string(), node.key.clone()]
            }
        }
        _ => vec![node.encode()],
    }
}
