use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, PanelDecl, UiNodeDecl};

/// First path segment of scene-scoped UI node keys (`home/panel/block`).
pub fn scene_id_from_ui_node_key(key: &str) -> Option<String> {
    key.split('/').next().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn preview_target_for_scene_id(
    compiled: &CompiledApp,
    scene_id: &str,
) -> Option<String> {
    compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == scene_id)
        .map(|route| route.target_file.clone())
}

pub fn preview_target_from_build_node_with_app(
    node: &BuildNodeId,
    compiled: Option<&CompiledApp>,
) -> Option<String> {
    match node.kind {
        BuildNodeKind::WorldFile => Some(node.key.clone()),
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric => {
            let (file, _) = split_file_symbol(&node.key);
            non_empty_path(file)
        }
        BuildNodeKind::WorldExplain => {
            let (file, _, _) = split_world_explain_key(&node.key);
            non_empty_path(file)
        }
        BuildNodeKind::Scene | BuildNodeKind::Route => compiled
            .and_then(|app| preview_target_for_scene_id(app, node.key.as_str())),
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            let scene_id = scene_id_from_ui_node_key(&node.key)?;
            compiled.and_then(|app| preview_target_for_scene_id(app, scene_id.as_str()))
        }
        BuildNodeKind::Projection => {
            let scene_id = node.key.split('/').next()?;
            compiled.and_then(|app| preview_target_for_scene_id(app, scene_id))
        }
        _ => None,
    }
}

pub fn compile_scene_from_build_node(node: &BuildNodeId) -> Option<String> {
    match node.kind {
        BuildNodeKind::Scene | BuildNodeKind::Route => non_empty_path(node.key.clone()),
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock | BuildNodeKind::Projection => {
            scene_id_from_ui_node_key(&node.key)
        }
        _ => None,
    }
}

/// Human-readable breadcrumb segments for build overview / agent export.
pub fn build_experience_path(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    match node.kind {
        BuildNodeKind::Route | BuildNodeKind::Scene => scene_label(compiled, &node.key),
        BuildNodeKind::Projection => {
            let (scene_id, projection_id) = split_projection_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            path.push(projection_label(projection_id.as_str()));
            path
        }
        BuildNodeKind::ScenePanel => {
            let (scene_id, panel_id) = split_panel_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel(compiled, &scene_id, panel_id.as_str()) {
                path.push(panel_label(&panel));
            } else {
                path.push(panel_id);
            }
            path
        }
        BuildNodeKind::SceneBlock => {
            let (scene_id, panel_id, block_id) = split_block_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel(compiled, &scene_id, panel_id.as_str()) {
                path.push(panel_label(&panel));
                if let Some(block) = find_block_in_panel(&panel, block_id.as_str()) {
                    path.push(block_label(&block));
                } else {
                    path.push(block_id);
                }
            } else {
                path.push(panel_id);
                path.push(block_id);
            }
            path
        }
        BuildNodeKind::WorldFile => vec!["Backing · World".to_string(), node.key.clone()],
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric => {
            let (file, symbol) = split_file_symbol(&node.key);
            vec![
                "Backing · World".to_string(),
                file,
                symbol,
            ]
        }
        BuildNodeKind::WorldExplain => {
            let (file, metric, explain) = split_world_explain_key(&node.key);
            vec![
                "Backing · World".to_string(),
                file,
                metric,
                explain,
            ]
        }
        BuildNodeKind::Dataset => vec!["Backing · Datasets".to_string(), node.key.clone()],
        _ => vec![node.encode()],
    }
}

pub fn backing_refs_from_block_props(props: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_backing_refs(props, &mut refs);
    dedupe_preserve_order(&mut refs);
    refs
}

pub fn aggregate_use_key_badges(blocks: &[UiNodeDecl]) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for block in blocks {
        if let UiNodeDecl::Block(block) = block {
            *counts.entry(block.use_key.clone()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(use_key, count)| {
            if count > 1 {
                format!("{use_key} ×{count}")
            } else {
                use_key
            }
        })
        .collect()
}

fn scene_label(compiled: &CompiledApp, scene_id: &str) -> Vec<String> {
    let label = compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == scene_id)
        .and_then(|route| route.title.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| scene_id.to_string());
    vec![label]
}

fn projection_label(projection_id: &str) -> String {
    format!("Board · {projection_id}")
}

fn panel_label(panel: &PanelDecl) -> String {
    panel
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panel.id.clone())
}

fn block_label(block: &BlockDecl) -> String {
    block
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            block
                .id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| block.use_key.clone())
        })
}

fn find_panel(compiled: &CompiledApp, scene_id: &str, panel_id: &str) -> Option<PanelDecl> {
    panels_for_scene(compiled, scene_id)?
        .into_iter()
        .find(|panel| panel.id == panel_id)
}

fn find_block_in_panel(panel: &PanelDecl, block_id: &str) -> Option<BlockDecl> {
    panel.blocks.iter().find_map(|node| match node {
        UiNodeDecl::Block(block) => {
            let id = block
                .id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(block.use_key.as_str());
            if id == block_id {
                Some(block.clone())
            } else {
                None
            }
        }
        _ => None,
    })
}

pub fn panels_for_scene(compiled: &CompiledApp, scene_id: &str) -> Option<Vec<PanelDecl>> {
    compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(|assembly| assembly.get("panels"))
        .and_then(|value| serde_json::from_value::<Vec<PanelDecl>>(value.clone()).ok())
}

fn split_panel_key(key: &str) -> (String, String) {
    let mut parts = key.splitn(2, '/');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

fn split_block_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '/');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

fn split_projection_key(key: &str) -> (String, String) {
    key.split_once('/')
        .map(|(scene, projection)| (scene.to_string(), projection.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

fn split_file_symbol(key: &str) -> (String, String) {
    key.split_once('#')
        .map(|(file, symbol)| (file.to_string(), symbol.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

fn split_world_explain_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '#');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

fn non_empty_path(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn collect_backing_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(ref_kind) = map.get("__ref").and_then(Value::as_str) {
                match ref_kind {
                    "data" | "dataset" | "resource" | "entity" => {
                        if let Some(id) = map
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            out.push(format!("→ {id}"));
                        }
                    }
                    "metric" => {
                        if let Some(id) = map.get("id").and_then(Value::as_str) {
                            let from = map
                                .get("from_dataset")
                                .or_else(|| map.get("from"))
                                .and_then(Value::as_str);
                            if let Some(dataset) = from.filter(|s| !s.trim().is_empty()) {
                                out.push(format!("→ {dataset}::{id}"));
                            } else {
                                out.push(format!("→ metric:{id}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for child in map.values() {
                collect_backing_refs(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_backing_refs(item, out);
            }
        }
        _ => {}
    }
}

pub fn build_overview_backing(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    use BuildNodeKind::*;
    match node.kind {
        SceneBlock => {
            let (scene_id, panel_id, block_id) = split_block_key(&node.key);
            let Some(panel) = find_panel(compiled, &scene_id, panel_id.as_str()) else {
                return Vec::new();
            };
            let Some(block) = find_block_in_panel(&panel, block_id.as_str()) else {
                return Vec::new();
            };
            backing_refs_from_block_props(&block.props)
        }
        ScenePanel => {
            let (scene_id, panel_id) = split_panel_key(&node.key);
            let Some(panel) = find_panel(compiled, &scene_id, panel_id.as_str()) else {
                return Vec::new();
            };
            let mut refs = Vec::new();
            for ui_node in &panel.blocks {
                if let UiNodeDecl::Block(block) = ui_node {
                    refs.extend(backing_refs_from_block_props(&block.props));
                }
            }
            dedupe_preserve_order(&mut refs);
            refs
        }
        WorldDataset | WorldMetric => {
            vec![format!("world: {}", node.key.replace('#', " › "))]
        }
        Dataset => vec![format!("resource: {}", node.key)],
        _ => Vec::new(),
    }
}

pub fn format_experience_path(path: &[String]) -> String {
    path.join(" › ")
}

fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = BTreeMap::<String, ()>::new();
    items.retain(|item| {
        if seen.contains_key(item) {
            false
        } else {
            seen.insert(item.clone(), ());
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backing_refs_from_metric_binding() {
        let props = serde_json::json!({
            "metric": { "__ref": "metric", "id": "total", "from_dataset": "agency_objects" }
        });
        let refs = backing_refs_from_block_props(&props);
        assert!(refs.iter().any(|r| r.contains("agency_objects")));
    }

    #[test]
    fn compile_scene_from_panel_node() {
        let node = BuildNodeId::scene_panel("home", "kpi_row");
        assert_eq!(compile_scene_from_build_node(&node).as_deref(), Some("home"));
    }
}
