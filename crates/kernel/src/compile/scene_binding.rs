use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::{FrameDecl, SceneDecl};

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

pub(super) fn parse_scene_binding(
    value: &Value,
    expected_kind: &str,
    label: &str,
) -> Result<SceneBinding> {
    if value.is_null() {
        return Ok(SceneBinding::Absent);
    }
    if let Some(id) = value.as_str().map(str::trim) {
        if id.is_empty() {
            return Ok(SceneBinding::Absent);
        }
        return Ok(SceneBinding::LocalId(id.to_string()));
    }
    if expected_kind == "world_file_ref" {
        let world_ref = serde_json::from_value::<WorldFileRefDecl>(value.clone())
            .map_err(|error| anyhow!("invalid {label} binding: {error}"))?;
        if world_ref.kind != expected_kind {
            return Err(anyhow!(
                "invalid {label} binding kind `{}`, expected `{expected_kind}`",
                world_ref.kind
            ));
        }
        if world_ref.path.trim().is_empty() {
            return Err(anyhow!("{label}_file_ref path must not be empty"));
        }
        return Ok(SceneBinding::FileRef {
            path: world_ref.path,
            id: world_ref.id,
        });
    }
    if expected_kind == "frame_file_ref" {
        let frame_ref = serde_json::from_value::<FrameFileRefDecl>(value.clone())
            .map_err(|error| anyhow!("invalid {label} binding: {error}"))?;
        if frame_ref.kind != expected_kind {
            return Err(anyhow!(
                "invalid {label} binding kind `{}`, expected `{expected_kind}`",
                frame_ref.kind
            ));
        }
        if frame_ref.path.trim().is_empty() {
            return Err(anyhow!("{label}_file_ref path must not be empty"));
        }
        return Ok(SceneBinding::FileRef {
            path: frame_ref.path,
            id: frame_ref.id,
        });
    }
    Err(anyhow!(
        "unsupported {label} binding; expected local id string or {expected_kind}(...)"
    ))
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
