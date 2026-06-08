use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::compile::load_external::load_flow_from_file;
use crate::compile::scene_binding::{parse_flow_binding, SceneBinding};
use crate::model::{Diagnostic, FlowDecl, FrameDecl, PanelDecl, Severity};
use crate::typed_refs::SceneRegistry;

use super::compile_scene::push_deprecated_ref_binding_diagnostic;
use super::super::clone_merge::{normalize_flow_decl, resolve_panel_slot};

pub(super) fn resolve_flow_binding(
    value: &Value,
    flows: &BTreeMap<String, FlowDecl>,
    app_root: &Path,
    scene_registry: Option<&SceneRegistry>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FlowDecl> {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        if let Some(flow) = flows.get(id) {
            return Some(flow.clone());
        }
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_bound_flow".to_string(),
            message: format!("declared flow `{id}` was not found"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    match parse_flow_binding(value, scene_registry) {
        Ok(SceneBinding::LocalId(id)) => {
            if let Some(flow) = flows.get(id.as_str()) {
                return Some(flow.clone());
            }
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_flow".to_string(),
                message: format!("declared flow `{id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
            None
        }
        Ok(SceneBinding::FileRef {
            path,
            id,
            compat_source,
        }) => {
            push_deprecated_ref_binding_diagnostic(
                diagnostics,
                compat_source.as_deref(),
                target_file,
            );
            match load_flow_from_file(app_root, path.as_str(), id.as_deref()) {
                Ok(flow_decl) => {
                    let Some(registry) = scene_registry else {
                        return Some(flow_decl);
                    };
                    let flow_value = serde_json::to_value(&flow_decl).ok()?;
                    normalize_flow_decl(
                        app_root,
                        flow_decl,
                        &flow_value,
                        registry,
                        diagnostics,
                        target_file,
                    )
                }
                Err(error) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "load_flow_ref_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    });
                    None
                }
            }
        }
        Ok(SceneBinding::Absent) => None,
        Err(message) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_flow_binding".to_string(),
                message: message.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(super) fn merge_frame_panel_slots(
    app_root: &Path,
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<&FrameDecl>,
    panels: &mut Vec<PanelDecl>,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) {
    let mut sources: Vec<&FrameDecl> = frames.values().collect();
    if let Some(frame) = frame_default {
        sources.push(frame);
    }
    for frame in sources {
        for slot in &frame.panels {
            if let Some(panel) =
                resolve_panel_slot(app_root, slot, scene_registry, diagnostics, target_file)
            {
                upsert_panel(panels, panel);
            }
        }
    }
}

fn upsert_panel(panels: &mut Vec<PanelDecl>, panel: PanelDecl) {
    if let Some(existing) = panels.iter_mut().find(|item| item.id == panel.id) {
        *existing = panel;
        return;
    }
    panels.push(panel);
}
