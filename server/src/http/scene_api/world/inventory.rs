use std::fs;
use std::path::Path;

use mei_lang_kernel::{PanelDecl, UiNodeDecl};
use serde_json::Value;

use crate::http::scene_api::types::{
    ResourceInventoryItem, ResourceInventorySnapshot, WorldRuntimeBundle, WorldScope,
};
use super::util::normalize_path;

fn file_ref_from_scene_binding(value: Option<&Value>, expected_kind: &str) -> Option<String> {
    let value = value?;
    let map = value.as_object()?;
    let kind = map
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if kind != expected_kind {
        return None;
    }
    let path = map
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if path.is_empty() {
        return None;
    }
    Some(normalize_path(path))
}

pub(crate) fn related_to_target(source_path: Option<&str>, target_file: Option<&str>) -> bool {
    let Some(target) = target_file
        .map(normalize_path)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    source_path
        .map(normalize_path)
        .is_some_and(|source| source == target)
}

fn collect_refs_from_value(value: &Value, refs: &mut Vec<String>, depth: usize) {
    if depth > 5 {
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(kind) = map.get("kind").and_then(Value::as_str) {
                if kind.ends_with("_ref") || kind.ends_with("_file_ref") {
                    refs.push(kind.to_string());
                }
            }
            if let Some(raw_ref) = map.get("__ref").and_then(Value::as_str) {
                refs.push(format!("__ref:{raw_ref}"));
            }
            for (key, entry) in map {
                if key.ends_with("_ref") || key.ends_with("_file_ref") {
                    if let Some(text) = entry.as_str() {
                        refs.push(format!("{key}:{text}"));
                    } else {
                        refs.push(key.to_string());
                    }
                }
                collect_refs_from_value(entry, refs, depth + 1);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs_from_value(item, refs, depth + 1);
            }
        }
        _ => {}
    }
}

pub(crate) fn extract_ref_tokens_from_source(source: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        for token in [
            "world_ref(",
            "frame_ref(",
            "panel_ref(",
            "data_ref(",
            "metric_ref(",
        ] {
            if trimmed.contains(token) {
                refs.push(token.trim_end_matches('(').to_string());
            }
        }
        for token in ["scene_file_ref(", "world_file_ref(", "frame_file_ref("] {
            if trimmed.contains(token) {
                refs.push(token.trim_end_matches('(').to_string());
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn collect_panel_references(panel: &PanelDecl) -> Vec<String> {
    let mut refs = Vec::new();
    for node in &panel.blocks {
        match node {
            UiNodeDecl::Panel(child) => {
                refs.push(format!("panel:{}", child.id));
                refs.extend(collect_panel_references(child));
            }
            UiNodeDecl::Block(block) => {
                refs.push(format!("use_key:{}", block.use_key));
                collect_refs_from_value(&block.props, &mut refs, 0);
            }
            UiNodeDecl::FrameRef(frame_ref) => {
                refs.push(format!("frame_ref:{}", frame_ref.frame_ref));
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn push_inventory_item(
    out: &mut Vec<ResourceInventoryItem>,
    id: String,
    resource_type: &str,
    title: Option<String>,
    summary: Option<String>,
    source_path: Option<String>,
    references: Vec<String>,
    target_file: Option<&str>,
) {
    out.push(ResourceInventoryItem {
        id,
        resource_type: resource_type.to_string(),
        title,
        summary,
        source_path: source_path.clone(),
        references,
        related_to_target: related_to_target(source_path.as_deref(), target_file),
    });
}

pub(super) fn build_resource_inventory(
    source_root: &Path,
    app_id: &str,
    bundle: &WorldRuntimeBundle,
    scope: Option<&WorldScope>,
) -> ResourceInventorySnapshot {
    let target_file = scope
        .and_then(|item| item.target_file.as_deref())
        .map(normalize_path)
        .or_else(|| Some(bundle.active_target_file.clone()));
    let target_ref = target_file.as_deref();
    let scene_world_file_ref =
        file_ref_from_scene_binding(bundle.contract.scene.world.as_ref(), "world_file_ref");
    let scene_frame_file_ref =
        file_ref_from_scene_binding(bundle.contract.scene.frame.as_ref(), "frame_file_ref");
    let mut items = Vec::new();

    push_inventory_item(
        &mut items,
        bundle.contract.scene.id.clone(),
        "scene",
        Some(bundle.contract.scene.id.clone()),
        bundle.contract.scene.summary.clone(),
        Some(bundle.active_target_file.clone()),
        Vec::new(),
        target_ref,
    );
    if let Some(path) = scene_world_file_ref.clone() {
        push_inventory_item(
            &mut items,
            format!("world_file_ref:{path}"),
            "world_file_ref",
            Some(path.clone()),
            Some("scene 绑定的外部 world 文件".to_string()),
            Some(path),
            Vec::new(),
            target_ref,
        );
    }
    if let Some(path) = scene_frame_file_ref.clone() {
        push_inventory_item(
            &mut items,
            format!("frame_file_ref:{path}"),
            "frame_file_ref",
            Some(path.clone()),
            Some("scene 绑定的外部 frame 文件".to_string()),
            Some(path),
            Vec::new(),
            target_ref,
        );
    }

    if let Some(world) = &bundle.contract.world {
        push_inventory_item(
            &mut items,
            world.id.clone().unwrap_or_else(|| "world".to_string()),
            "world",
            world.id.clone(),
            Some(format!(
                "resources={} entities={} cells={}",
                world.resources.len(),
                world.entities.len(),
                world
                    .topology
                    .as_ref()
                    .map(|item| item.cells.len())
                    .unwrap_or(0)
            )),
            scene_world_file_ref
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
            Vec::new(),
            target_ref,
        );
        for item in &world.resources {
            let mut references = Vec::new();
            if let Some(source) = item.source.as_ref() {
                if !source.path.trim().is_empty() {
                    references.push(format!("source_path:{}", normalize_path(&source.path)));
                }
            }
            push_inventory_item(
                &mut items,
                item.id.clone(),
                "resource",
                item.title.clone(),
                Some(format!("kind={}", item.kind)),
                item.source
                    .as_ref()
                    .map(|source| normalize_path(&source.path))
                    .or_else(|| scene_world_file_ref.clone())
                    .or_else(|| Some(bundle.active_target_file.clone())),
                references,
                target_ref,
            );
        }
        for item in &world.entities {
            push_inventory_item(
                &mut items,
                item.id.clone(),
                "entity",
                item.label.clone(),
                Some(format!(
                    "kind={} status={}",
                    item.kind,
                    item.status.as_deref().unwrap_or("unknown")
                )),
                scene_world_file_ref
                    .clone()
                    .or_else(|| Some(bundle.active_target_file.clone())),
                item.spawns.clone(),
                target_ref,
            );
        }
        if let Some(topology) = &world.topology {
            for cell in &topology.cells {
                push_inventory_item(
                    &mut items,
                    cell.id.clone(),
                    "cell",
                    cell.surface_kind.clone(),
                    Some(format!(
                        "hazard={} row={:?} col={:?}",
                        cell.hazard_state.as_deref().unwrap_or("none"),
                        cell.row,
                        cell.col
                    )),
                    scene_world_file_ref
                        .clone()
                        .or_else(|| Some(bundle.active_target_file.clone())),
                    cell.tags.clone(),
                    target_ref,
                );
            }
        }
    }

    if let Some(frame) = &bundle.contract.frame {
        push_inventory_item(
            &mut items,
            frame.id.clone().unwrap_or_else(|| "frame".to_string()),
            "frame",
            frame.title.clone(),
            Some("scene 主 frame".to_string()),
            scene_frame_file_ref
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
            Vec::new(),
            target_ref,
        );
    }
    if let Some(flow) = &bundle.contract.flow {
        push_inventory_item(
            &mut items,
            flow.id.clone().unwrap_or_else(|| "flow".to_string()),
            "flow",
            flow.id.clone(),
            Some(format!(
                "interactions={} subject_timers={}",
                flow.interactions.len(),
                flow.subject_timers.len()
            )),
            Some(bundle.active_target_file.clone()),
            Vec::new(),
            target_ref,
        );
    }
    for panel in &bundle.contract.panels {
        push_inventory_item(
            &mut items,
            panel.id.clone(),
            "panel",
            panel.title.clone(),
            Some(format!("blocks={}", panel.blocks.len())),
            scene_frame_file_ref
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
            collect_panel_references(panel),
            target_ref,
        );
    }

    for route in &bundle.compiled.scene_routes {
        push_inventory_item(
            &mut items,
            route.scene_id.clone(),
            "scene_route",
            route.title.clone(),
            Some(format!("kind={}", route.kind)),
            Some(normalize_path(&route.target_file)),
            vec![format!("scene:{}", route.scene_id)],
            target_ref,
        );
    }
    for resource in &bundle.compiled.resources {
        push_inventory_item(
            &mut items,
            resource.id.clone(),
            "loaded_resource",
            resource.title.clone(),
            Some(format!(
                "kind={} dataset={}",
                resource.kind,
                if resource.dataset.is_some() {
                    "yes"
                } else {
                    "no"
                }
            )),
            None,
            Vec::new(),
            target_ref,
        );
    }
    for asset in &bundle.compiled.component_assets {
        push_inventory_item(
            &mut items,
            asset.key.clone(),
            "component_asset",
            Some(asset.tag.clone()),
            Some(format!("script={}", asset.script)),
            Some(normalize_path(&asset.script)),
            Vec::new(),
            target_ref,
        );
    }

    if let Some(target) = target_ref {
        let source_path = source_root.join(app_id).join(target);
        if let Ok(source) = fs::read_to_string(&source_path) {
            let refs = extract_ref_tokens_from_source(&source);
            if !refs.is_empty() {
                push_inventory_item(
                    &mut items,
                    format!("source_refs:{target}"),
                    "source_refs",
                    Some(target.to_string()),
                    Some("当前文件中检测到的 *_ref/*_file_ref 引用提示".to_string()),
                    Some(target.to_string()),
                    refs,
                    target_ref,
                );
            }
        }
    }

    ResourceInventorySnapshot {
        target_file,
        total_items: items.len(),
        items,
    }
}
