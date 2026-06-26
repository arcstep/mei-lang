use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::{
    FrameDecl, FrameExportDecl, PanelDecl,
    PanelExportDecl, SceneDecl, SceneExportDecl,
};

use super::super::scene_binding::decode_scene_decl;

pub(crate) fn set_missing_id(value: &mut Value, id: &str) {
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

pub(crate) fn load_scene_decl_values(
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

pub(crate) fn load_frame_decl_values(decls: &Value) -> Result<Vec<FrameDecl>> {
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

pub(crate) fn load_panel_decl_values(decls: &Value) -> Result<Vec<PanelDecl>> {
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
