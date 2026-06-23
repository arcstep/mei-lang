use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::{
    ComponentExportDecl, EntityDecl, FlowDecl, FrameDecl, FrameExportDecl, PanelDecl,
    PanelExportDecl, ResourceDecl, SceneDecl, SceneExportDecl,
};

use super::decl_file_cache::evaluate_mei_file_cached;
use super::decls::{
    FrameSetLayoutDecl, WorldAddEntityDecl, WorldAddMetricDecl, WorldAddResourceDecl,
    WorldSetTopologyDecl,
};
use super::mutations::apply_world_mutations_to_decl;
use super::scene_binding::{decode_scene_decl, parse_world_binding, SceneBinding};

fn set_missing_id(value: &mut Value, id: &str) {
    let should_fill = value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|value| value.is_empty())
        .unwrap_or(true);
    if should_fill {
        value["id"] = Value::String(id.to_string());
    }
}

fn load_scene_decl_values(
    app_root: &Path,
    relative_path: &str,
    decls: &Value,
) -> Result<Vec<SceneDecl>> {
    let mut scenes = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("scene") => {
                    scenes.push(decode_scene_decl(app_root, value, relative_path, None)?)
                }
                Some("scene_export") => {
                    let export = serde_json::from_value::<SceneExportDecl>(value.clone())?;
                    let mut scene_value = export.scene;
                    set_missing_id(&mut scene_value, export.id.as_str());
                    scenes.push(decode_scene_decl(
                        app_root,
                        &scene_value,
                        relative_path,
                        None,
                    )?);
                }
                _ => {}
            }
        }
    }
    Ok(scenes)
}

fn load_frame_decl_values(decls: &Value) -> Result<Vec<FrameDecl>> {
    let mut frames = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("frame") => {
                    frames.push(serde_json::from_value::<FrameDecl>(value.clone())?);
                }
                Some("frame_export") => {
                    let export = serde_json::from_value::<FrameExportDecl>(value.clone())?;
                    let mut frame_value = export.frame;
                    set_missing_id(&mut frame_value, export.id.as_str());
                    frames.push(serde_json::from_value::<FrameDecl>(frame_value)?);
                }
                _ => {}
            }
        }
    }
    Ok(frames)
}

fn load_panel_decl_values(decls: &Value) -> Result<Vec<PanelDecl>> {
    let mut panels = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("panel") | Some("panel_decl") => {
                    if let Ok(panel) = serde_json::from_value::<PanelDecl>(value.clone()) {
                        panels.push(panel);
                    }
                }
                Some("panel_export") => {
                    let export = serde_json::from_value::<PanelExportDecl>(value.clone())?;
                    let mut panel_value = export.panel;
                    set_missing_id(&mut panel_value, export.id.as_str());
                    if let Ok(panel) = serde_json::from_value::<PanelDecl>(panel_value) {
                        panels.push(panel);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(panels)
}

pub(super) fn load_world_from_file(
    app_root: &Path,
    relative_path: &str,
    world_id: Option<&str>,
) -> Result<crate::model::WorldDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    let mut worlds = Vec::new();
    let mut pending_resources = Vec::new();
    let mut pending_entities = Vec::new();
    let mut pending_metrics = Vec::new();
    let mut pending_topology = None;
    let mut world_topology_set_count = 0usize;
    let mut seen_world_decl = false;
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("world") => {
                    worlds.push(serde_json::from_value::<crate::model::WorldDecl>(
                        value.clone(),
                    )?);
                    seen_world_decl = true;
                }
                Some("world_add_resource") => {
                    if !seen_world_decl {
                        return Err(anyhow!(
                            "world_file_ref `{relative_path}`: `world.add_*` / `world.set_topology(...)` must appear after `world(...)` (_declare order)"
                        ));
                    }
                    let decl = serde_json::from_value::<WorldAddResourceDecl>(value.clone())?;
                    pending_resources.push(decl.resource);
                }
                Some("world_add_entity") => {
                    if !seen_world_decl {
                        return Err(anyhow!(
                            "world_file_ref `{relative_path}`: `world.add_*` / `world.set_topology(...)` must appear after `world(...)` (_declare order)"
                        ));
                    }
                    let decl = serde_json::from_value::<WorldAddEntityDecl>(value.clone())?;
                    pending_entities.push(decl.entity);
                }
                Some("world_add_metric") => {
                    if !seen_world_decl {
                        return Err(anyhow!(
                            "world_file_ref `{relative_path}`: `world.add_*` / `world.set_topology(...)` must appear after `world(...)` (_declare order)"
                        ));
                    }
                    let decl = serde_json::from_value::<WorldAddMetricDecl>(value.clone())?;
                    pending_metrics.push(decl.metric);
                }
                Some("world_set_topology") => {
                    if !seen_world_decl {
                        return Err(anyhow!(
                            "world_file_ref `{relative_path}`: `world.add_*` / `world.set_topology(...)` must appear after `world(...)` (_declare order)"
                        ));
                    }
                    let decl = serde_json::from_value::<WorldSetTopologyDecl>(value.clone())?;
                    world_topology_set_count += 1;
                    if pending_topology.is_none() {
                        pending_topology = Some(decl.topology);
                    }
                }
                _ => {}
            }
        }
    }
    if world_topology_set_count > 1 {
        return Err(anyhow!(
            "world_file_ref `{relative_path}` declared multiple world.set_topology(...) blocks"
        ));
    }
    if !pending_resources.is_empty()
        || !pending_entities.is_empty()
        || !pending_metrics.is_empty()
        || pending_topology.is_some()
    {
        match worlds.len() {
            0 => {
                return Err(anyhow!(
                    "world_file_ref `{relative_path}` used world.add_* / world.set_topology(...) without world(...)"
                ));
            }
            1 => {
                if let Some(world_decl) = worlds.first_mut() {
                    apply_world_mutations_to_decl(
                        world_decl,
                        &pending_resources,
                        &pending_entities,
                        &pending_metrics,
                        pending_topology,
                    );
                }
            }
            count => {
                return Err(anyhow!(
                    "world_file_ref `{relative_path}` used world.add_* / world.set_topology(...) with {count} world(...) declarations"
                ));
            }
        }
    }
    if let Some(expected_id) = world_id {
        return worlds
            .into_iter()
            .find(|decl| decl.id.as_deref() == Some(expected_id))
            .ok_or_else(|| {
                anyhow!("world_file_ref `{relative_path}` did not contain world id `{expected_id}`")
            });
    }
    match worlds.len() {
        0 => Err(anyhow!(
            "world_file_ref `{relative_path}` did not contain world(...) declarations"
        )),
        1 => worlds
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("world_file_ref `{relative_path}` did not contain world")),
        count => Err(anyhow!(
            "world_file_ref `{relative_path}` matched {count} world(...) declarations; provide id"
        )),
    }
}

/// 解析 scene capsule 的 world：同文件 `world(...)`，或 `scene(world = world_ref(...))` 外引。
pub(super) fn load_world_for_capsule_file(
    app_root: &Path,
    scene_relative_path: &str,
) -> Result<crate::model::WorldDecl> {
    if let Ok(decl) = load_world_from_file(app_root, scene_relative_path, None) {
        return Ok(decl);
    }
    let scene = load_scene_from_file(app_root, scene_relative_path, None)?;
    let Some(world_slot) = scene.world.as_ref() else {
        return Err(anyhow!(
            "capsule `{scene_relative_path}` has no world(...) or scene(world = world_ref(...))"
        ));
    };
    match parse_world_binding(world_slot, None)? {
        SceneBinding::Absent => Err(anyhow!(
            "capsule `{scene_relative_path}` scene(...) has no world binding"
        )),
        SceneBinding::LocalId(world_id) => {
            load_world_from_file(app_root, scene_relative_path, Some(world_id.as_str()))
        }
        SceneBinding::FileRef { path, id, .. } => {
            load_world_from_file(app_root, path.as_str(), id.as_deref())
        }
    }
}

pub(super) fn load_frame_from_file(
    app_root: &Path,
    relative_path: &str,
    frame_id: Option<&str>,
) -> Result<FrameDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    let mut frames = load_frame_decl_values(&decls)?;
    let mut pending_layout: Option<crate::model::LayoutDecl> = None;
    let mut frame_layout_set_count = 0usize;
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("frame_set_layout") => {
                    let decl = serde_json::from_value::<FrameSetLayoutDecl>(value.clone())?;
                    frame_layout_set_count += 1;
                    if pending_layout.is_none() {
                        pending_layout = Some(serde_json::from_value::<crate::model::LayoutDecl>(
                            decl.layout,
                        )?);
                    }
                }
                _ => {}
            }
        }
    }
    if frame_layout_set_count > 1 {
        return Err(anyhow!(
            "frame_file_ref `{relative_path}` declared multiple frame.set_layout(...) blocks"
        ));
    }
    if let Some(layout) = pending_layout {
        match frames.len() {
            0 => {
                return Err(anyhow!(
                    "frame_file_ref `{relative_path}` used frame.set_layout(...) without frame(...)"
                ));
            }
            1 => {
                if let Some(frame_decl) = frames.first_mut() {
                    frame_decl.layout = Some(layout);
                }
            }
            count => {
                return Err(anyhow!(
                    "frame_file_ref `{relative_path}` used frame.set_layout(...) with {count} frame(...) declarations"
                ));
            }
        }
    }
    if let Some(expected_id) = frame_id {
        return frames
            .into_iter()
            .find(|decl| decl.id.as_deref() == Some(expected_id))
            .ok_or_else(|| {
                anyhow!("frame_file_ref `{relative_path}` did not contain frame id `{expected_id}`")
            });
    }
    match frames.len() {
        0 => Err(anyhow!(
            "frame_file_ref `{relative_path}` did not contain frame(...) declarations"
        )),
        1 => frames
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("frame_file_ref `{relative_path}` did not contain frame")),
        count => Err(anyhow!(
            "frame_ref `{relative_path}` matched {count} frame(...) declarations; provide id"
        )),
    }
}

pub(super) fn load_flow_from_file(
    app_root: &Path,
    relative_path: &str,
    flow_id: Option<&str>,
) -> Result<FlowDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    let mut flows = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("flow") {
                flows.push(serde_json::from_value::<FlowDecl>(value.clone())?);
            }
        }
    }
    if let Some(expected_id) = flow_id {
        return flows
            .into_iter()
            .find(|decl| decl.id.as_deref() == Some(expected_id))
            .ok_or_else(|| {
                anyhow!("flow_ref `{relative_path}` did not contain flow id `{expected_id}`")
            });
    }
    match flows.len() {
        0 => Err(anyhow!(
            "flow_ref `{relative_path}` did not contain flow(...) declarations"
        )),
        1 => flows
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("flow_ref `{relative_path}` did not contain flow")),
        count => Err(anyhow!(
            "flow_ref `{relative_path}` matched {count} flow(...) declarations; provide id"
        )),
    }
}

pub(super) fn load_panel_from_scene_file(
    app_root: &Path,
    relative_path: &str,
    panel_id: &str,
) -> Result<PanelDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    load_panel_decl_values(&decls)?
        .into_iter()
        .find(|panel| panel.id == panel_id)
        .ok_or_else(|| anyhow!("panel_ref `{relative_path}` did not contain panel id `{panel_id}`"))
}

pub(super) fn load_scene_decls_from_file(
    app_root: &Path,
    relative_path: &str,
) -> Result<Vec<SceneDecl>> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    load_scene_decl_values(app_root, relative_path, &decls)
}

pub(super) fn load_scene_from_file(
    app_root: &Path,
    relative_path: &str,
    scene_id: Option<&str>,
) -> Result<SceneDecl> {
    let scenes = load_scene_decls_from_file(app_root, relative_path)?;
    if let Some(expected_id) = scene_id {
        return scenes
            .into_iter()
            .find(|decl| decl.id == expected_id)
            .ok_or_else(|| {
                anyhow!("scene_ref `{relative_path}` did not contain scene id `{expected_id}`")
            });
    }
    match scenes.len() {
        0 => Err(anyhow!(
            "scene_ref `{relative_path}` did not contain scene(...) declarations"
        )),
        1 => scenes
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("scene_ref `{relative_path}` did not contain scene")),
        count => Err(anyhow!(
            "scene_ref `{relative_path}` matched {count} scene(...) declarations; provide id"
        )),
    }
}

fn find_resource_in_world(
    world: &crate::model::WorldDecl,
    resource_id: &str,
) -> Option<ResourceDecl> {
    all_resources(world)
        .into_iter()
        .find(|item| item.id == resource_id)
}

fn all_resources(world: &crate::model::WorldDecl) -> Vec<ResourceDecl> {
    let mut all = world.resources.clone();
    all.extend(world.datasets.clone());
    all.extend(world.metric_packs.clone());
    all
}

pub(super) fn load_resource_from_world_file(
    app_root: &Path,
    relative_path: &str,
    resource_id: &str,
    expected_kind: Option<&str>,
) -> Result<ResourceDecl> {
    let world = load_world_from_file(app_root, relative_path, None)?;
    let resource = find_resource_in_world(&world, resource_id).ok_or_else(|| {
        anyhow!(
            "resource_ref `{relative_path}` did not contain resource/dataset/metric id `{resource_id}`"
        )
    })?;
    if let Some(kind) = expected_kind {
        if resource.kind != kind {
            return Err(anyhow!(
                "resource_ref `{relative_path}` id `{resource_id}` has kind `{}`, expected `{kind}`",
                resource.kind
            ));
        }
    }
    Ok(resource)
}

pub(super) fn load_entity_from_world_file(
    app_root: &Path,
    relative_path: &str,
    entity_id: &str,
) -> Result<EntityDecl> {
    let world = load_world_from_file(app_root, relative_path, None)?;
    world
        .entities
        .into_iter()
        .find(|entity| entity.id == entity_id)
        .ok_or_else(|| {
            anyhow!("entity_ref `{relative_path}` did not contain entity id `{entity_id}`")
        })
}

pub(super) fn load_block_from_scene_file(
    app_root: &Path,
    relative_path: &str,
    block_id: Option<&str>,
    use_key: Option<&str>,
) -> Result<Value> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    let mut candidates = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            collect_block_candidates(value, &mut candidates);
        }
    }
    if let Some(id) = block_id.map(str::trim).filter(|id| !id.is_empty()) {
        if let Some(block) = candidates
            .iter()
            .find(|block| block.get("id").and_then(Value::as_str).map(str::trim) == Some(id))
        {
            return Ok(block.clone());
        }
        return Err(anyhow!(
            "component_ref `{relative_path}` did not contain block id `{id}`"
        ));
    }
    if let Some(use_key) = use_key.map(str::trim).filter(|key| !key.is_empty()) {
        let matches: Vec<&Value> = candidates
            .iter()
            .filter(|block| {
                block.get("use_key").and_then(Value::as_str).map(str::trim) == Some(use_key)
            })
            .collect();
        return match matches.len() {
            0 => Err(anyhow!(
                "component_ref `{relative_path}` did not contain component use `{use_key}`"
            )),
            1 => Ok(matches[0].clone()),
            count => Err(anyhow!(
                "component_ref `{relative_path}` matched {count} blocks for use `{use_key}`; provide id"
            )),
        };
    }
    Err(anyhow!(
        "component_ref `{relative_path}` requires block id or use"
    ))
}

fn collect_block_candidates(value: &Value, out: &mut Vec<Value>) {
    if value.get("kind").and_then(Value::as_str) == Some("component_export") {
        if let Ok(export) = serde_json::from_value::<ComponentExportDecl>(value.clone()) {
            let mut block = export.block;
            set_missing_id(&mut block, export.id.as_str());
            collect_block_candidates(&block, out);
        }
    }
    if value.get("kind").and_then(Value::as_str) == Some("panel_export") {
        if let Ok(export) = serde_json::from_value::<PanelExportDecl>(value.clone()) {
            let mut panel = export.panel;
            set_missing_id(&mut panel, export.id.as_str());
            collect_block_candidates(&panel, out);
        }
    }
    if value.get("kind").and_then(Value::as_str) == Some("block") || value.get("use_key").is_some()
    {
        out.push(value.clone());
    }
    if value.get("kind").and_then(Value::as_str) == Some("panel") {
        if let Some(blocks) = value.get("blocks").and_then(Value::as_array) {
            for block in blocks {
                collect_block_candidates(block, out);
            }
        }
    }
    if let Some(blocks) = value.get("blocks").and_then(Value::as_array) {
        for block in blocks {
            collect_block_candidates(block, out);
        }
    }
}
