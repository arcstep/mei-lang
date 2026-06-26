use super::{load_frame_decl_values, load_panel_decl_values, load_scene_decl_values};

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::{
    FlowDecl, FrameDecl, PanelDecl, SceneDecl,
};

use super::super::decl_file_cache::evaluate_mei_file_cached;
use super::super::decls::FrameSetLayoutDecl;

pub(crate) fn load_frame_from_file(
    app_root: &Path,
    relative_path: &str,
    frame_id: Option<&str>,
) -> Result<FrameDecl> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
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

pub(crate) fn load_flow_from_file(
    app_root: &Path,
    relative_path: &str,
    flow_id: Option<&str>,
) -> Result<FlowDecl> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
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

pub(crate) fn load_panel_from_scene_file(
    app_root: &Path,
    relative_path: &str,
    panel_id: &str,
) -> Result<PanelDecl> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    load_panel_decl_values(&decls)?
        .into_iter()
        .find(|panel| panel.id == panel_id)
        .ok_or_else(|| anyhow!("panel_ref `{relative_path}` did not contain panel id `{panel_id}`"))
}

pub(crate) fn load_scene_decls_from_file(
    app_root: &Path,
    relative_path: &str,
) -> Result<Vec<SceneDecl>> {
    let source_path = crate::mei_config::resolve_app_mei_file_path(app_root, relative_path);
    let decls = evaluate_mei_file_cached(&source_path)?;
    load_scene_decl_values(app_root, relative_path, &decls)
}

pub(crate) fn load_scene_from_file(
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
