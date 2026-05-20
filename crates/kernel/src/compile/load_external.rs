use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::eval::evaluate_mei_file;
use crate::model::{FlowDecl, FrameDecl, PanelDecl};

use super::decls::{
    FrameSetLayoutDecl, WorldAddEntityDecl, WorldAddResourceDecl, WorldSetTopologyDecl,
};
use super::mutations::apply_world_mutations_to_decl;

pub(super) fn load_world_from_file(
    app_root: &Path,
    relative_path: &str,
    world_id: Option<&str>,
) -> Result<crate::model::WorldDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file(&source_path)?;
    let mut worlds = Vec::new();
    let mut pending_resources = Vec::new();
    let mut pending_entities = Vec::new();
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
    if !pending_resources.is_empty() || !pending_entities.is_empty() || pending_topology.is_some() {
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

pub(super) fn load_frame_from_file(
    app_root: &Path,
    relative_path: &str,
    frame_id: Option<&str>,
) -> Result<FrameDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file(&source_path)?;
    let mut frames = Vec::new();
    let mut pending_layout: Option<crate::model::LayoutDecl> = None;
    let mut frame_layout_set_count = 0usize;
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("frame") => {
                    frames.push(serde_json::from_value::<FrameDecl>(value.clone())?);
                }
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
    let decls = evaluate_mei_file(&source_path)?;
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
    let decls = evaluate_mei_file(&source_path)?;
    let mut panels = Vec::new();
    if let Some(values) = decls.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("panel") {
                if let Ok(panel) = serde_json::from_value::<PanelDecl>(value.clone()) {
                    panels.push(panel);
                }
            }
        }
    }
    panels
        .into_iter()
        .find(|panel| panel.id == panel_id)
        .ok_or_else(|| {
            anyhow!("panel_ref `{relative_path}` did not contain panel id `{panel_id}`")
        })
}
