use super::{dedupe_preserve_order};

use serde_json::Value;

use crate::model::{
    BlockDecl, CompiledApp, PanelDecl,
    UiNodeDecl,
};

pub fn backing_refs_from_block_props(props: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_backing_refs(props, &mut refs);
    dedupe_preserve_order(&mut refs);
    refs
}

pub(super) fn scene_label(compiled: &CompiledApp, scene_id: &str) -> Vec<String> {
    let label = compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == scene_id)
        .and_then(|route| route.title.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| scene_id.to_string());
    vec![label]
}

pub(super) fn projection_label(projection_id: &str) -> String {
    format!("Board · {projection_id}")
}

pub(super) fn panel_label(panel: &PanelDecl) -> String {
    panel
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panel.id.clone())
}

pub fn block_instance_id(block: &BlockDecl, ordinal: usize) -> String {
    let stem = block
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| block.use_key.clone());
    format!("{stem}~{ordinal}")
}

pub(super) fn block_label(block: &BlockDecl) -> String {
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

pub(super) fn find_panel_by_path(
    compiled: &CompiledApp,
    scene_id: &str,
    panel_path: &str,
) -> Option<PanelDecl> {
    let top_level = panels_for_scene(compiled, scene_id)?;
    let mut segments = panel_path.split('/').filter(|s| !s.is_empty());
    let first = segments.next()?;
    let mut current = top_level.into_iter().find(|panel| panel.id == first)?;
    for segment in segments {
        current = current.blocks.iter().find_map(|node| match node {
            UiNodeDecl::Panel(panel) if panel.id == segment => Some(panel.clone()),
            _ => None,
        })?;
    }
    Some(current)
}

pub(super) fn find_block_in_panel(panel: &PanelDecl, block_id: &str) -> Option<BlockDecl> {
    for (ordinal, block) in blocks_in_panel(panel).iter().enumerate() {
        if block_instance_id(block, ordinal) == block_id {
            return Some((*block).clone());
        }
    }
    None
}

fn blocks_in_panel(panel: &PanelDecl) -> Vec<&BlockDecl> {
    panel
        .blocks
        .iter()
        .filter_map(|node| match node {
            UiNodeDecl::Block(block) => Some(block),
            _ => None,
        })
        .collect()
}

pub fn panels_for_scene(compiled: &CompiledApp, scene_id: &str) -> Option<Vec<PanelDecl>> {
    compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(|assembly| assembly.get("panels"))
        .and_then(|value| serde_json::from_value::<Vec<PanelDecl>>(value.clone()).ok())
}

pub(super) fn split_panel_key(key: &str) -> (String, String) {
    let mut parts = key.splitn(2, '/');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

pub(super) fn split_block_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '/');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

pub(super) fn split_projection_key(key: &str) -> (String, String) {
    key.split_once('/')
        .map(|(scene, projection)| (scene.to_string(), projection.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

pub(super) fn split_file_symbol(key: &str) -> (String, String) {
    key.split_once('#')
        .map(|(file, symbol)| (file.to_string(), symbol.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

pub(super) fn split_world_explain_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '#');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

pub(super) fn non_empty_path(value: String) -> Option<String> {
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

