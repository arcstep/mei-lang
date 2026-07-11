use super::{load_world_from_file, set_missing_id};

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::{ComponentExportDecl, EntityDecl, PanelExportDecl, ResourceDecl};

use super::super::decl_file_cache::evaluate_mei_file_cached;

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

pub(crate) fn load_resource_from_world_file(
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

pub(crate) fn load_entity_from_world_file(
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

pub(crate) fn load_block_from_scene_file(
    app_root: &Path,
    relative_path: &str,
    block_id: Option<&str>,
    use_key: Option<&str>,
) -> Result<Value> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
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
