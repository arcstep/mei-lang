use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::{FrameDecl, SceneDecl};
use crate::typed_refs::{decode_ref_value, RefKind, SceneRegistry};

use super::decls::{FrameFileRefDecl, WorldFileRefDecl};
use super::scene::scene_name_from_path;

pub(super) fn decode_scene_decl(value: &Value, target_file: &str) -> Result<SceneDecl> {
    let mut raw = value.clone();
    let missing_scene_id = raw
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|id| id.is_empty())
        .unwrap_or(true);
    if missing_scene_id {
        raw["id"] = Value::String(scene_name_from_path(target_file));
    }
    serde_json::from_value::<SceneDecl>(raw).map_err(Into::into)
}

#[derive(Debug, Clone)]
pub(super) enum SceneBinding {
    Absent,
    LocalId(String),
    FileRef { path: String, id: Option<String> },
}

pub(super) fn parse_world_binding(
    value: &Value,
    registry: Option<&SceneRegistry>,
) -> Result<SceneBinding> {
    parse_singleton_binding(value, RefKind::World, "world", registry)
}

pub(super) fn parse_frame_binding(
    value: &Value,
    registry: Option<&SceneRegistry>,
) -> Result<SceneBinding> {
    parse_singleton_binding(value, RefKind::Frame, "frame", registry)
}

pub(super) fn parse_flow_binding(
    value: &Value,
    registry: Option<&SceneRegistry>,
) -> Result<SceneBinding> {
    parse_singleton_binding(value, RefKind::Flow, "flow", registry)
}

fn parse_singleton_binding(
    value: &Value,
    expected: RefKind,
    label: &str,
    registry: Option<&SceneRegistry>,
) -> Result<SceneBinding> {
    if value.is_null() {
        return Ok(SceneBinding::Absent);
    }
    if let Some(expr) = decode_ref_value(value) {
        if expr.kind != expected {
            return Err(anyhow!(
                "invalid {label} binding: expected `{expected:?}` ref, got `{:?}`",
                expr.kind
            ));
        }
        if let Some(local_id) = expr
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
        {
            if expr.locator.scene_file.is_none() && expr.locator.scene_id.is_none() {
                return Ok(SceneBinding::LocalId(local_id));
            }
        }
        if expr.locator.scene_file.is_some() || expr.locator.scene_id.is_some() {
            let path = resolve_locator_path(&expr.locator, registry, label)?;
            return Ok(SceneBinding::FileRef {
                path,
                id: expr
                    .id
                    .as_deref()
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string),
            });
        }
        if let Some(local_id) = expr
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
        {
            return Ok(SceneBinding::LocalId(local_id));
        }
        return Err(anyhow!("{label}_ref requires id, scene_id, or scene_file"));
    }
    if let Some(id) = value.as_str().map(str::trim) {
        if id.is_empty() {
            return Ok(SceneBinding::Absent);
        }
        return Ok(SceneBinding::LocalId(id.to_string()));
    }
    if let Some(kind) = value.get("kind").and_then(Value::as_str) {
        let legacy_kind = match expected {
            RefKind::World => "world_file_ref",
            RefKind::Frame => "frame_file_ref",
            RefKind::Flow => "flow_file_ref",
            _ => "",
        };
        if kind == legacy_kind {
            if expected == RefKind::World {
                let world_ref = serde_json::from_value::<WorldFileRefDecl>(value.clone())?;
                if world_ref.path.trim().is_empty() {
                    return Err(anyhow!("world_file_ref path must not be empty"));
                }
                return Ok(SceneBinding::FileRef {
                    path: world_ref.path,
                    id: world_ref.id,
                });
            }
            if expected == RefKind::Frame {
                let frame_ref = serde_json::from_value::<FrameFileRefDecl>(value.clone())?;
                if frame_ref.path.trim().is_empty() {
                    return Err(anyhow!("frame_file_ref path must not be empty"));
                }
                return Ok(SceneBinding::FileRef {
                    path: frame_ref.path,
                    id: frame_ref.id,
                });
            }
        }
    }
    Err(anyhow!(
        "unsupported {label} binding; expected local id, {label}_ref(...), or legacy {label}_file_ref(...)"
    ))
}

fn resolve_locator_path(
    locator: &crate::typed_refs::SceneLocator,
    registry: Option<&SceneRegistry>,
    label: &str,
) -> Result<String> {
    if let Some(path) = locator
        .scene_file
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(path.to_string());
    }
    if let Some(registry) = registry {
        return registry
            .resolve_target(locator)
            .map(|(_, path)| path)
            .map_err(|message| anyhow!("{label}_ref: {message}"));
    }
    if let Some(scene_id) = locator.scene_id.as_deref().filter(|id| !id.is_empty()) {
        return Err(anyhow!(
            "{label}_ref(scene_id=`{scene_id}`) requires app scene registry context"
        ));
    }
    Err(anyhow!("{label}_ref requires scene_file or scene_id"))
}

pub(super) fn pick_only_frame(
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<FrameDecl>,
) -> Option<FrameDecl> {
    if frames.len() + usize::from(frame_default.is_some()) != 1 {
        return None;
    }
    frame_default.or_else(|| frames.values().next().cloned())
}

pub(super) fn pick_only_world(
    worlds: &BTreeMap<String, crate::model::WorldDecl>,
    world_default: Option<crate::model::WorldDecl>,
) -> Option<crate::model::WorldDecl> {
    if worlds.len() + usize::from(world_default.is_some()) != 1 {
        return None;
    }
    world_default.or_else(|| worlds.values().next().cloned())
}
