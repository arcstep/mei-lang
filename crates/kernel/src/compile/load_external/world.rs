use super::load_scene_from_file;

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::super::decl_file_cache::evaluate_mei_file_cached;
use super::super::decls::{
    WorldAddEntityDecl, WorldAddMetricDecl, WorldAddResourceDecl, WorldSetTopologyDecl,
};
use super::super::mutations::apply_world_mutations_to_decl;
use super::super::scene_binding::{parse_world_binding, SceneBinding};

pub(crate) fn load_world_from_file(
    app_root: &Path,
    relative_path: &str,
    world_id: Option<&str>,
) -> Result<crate::model::WorldDecl> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
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
pub(crate) fn load_world_for_capsule_file(
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
