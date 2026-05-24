use std::path::Path;

use serde_json::Value;

use crate::model::{Diagnostic, FrameDecl, PanelDecl, Severity, WorldDecl};
use crate::typed_refs::{decode_ref_value, RefKind, SceneRegistry};

use super::super::normalize::{
    normalize_entity_list, normalize_metric_list, normalize_resource_list, normalize_ui_nodes,
};
use crate::compile::panel_normalize::seed_metric_slot_vertical_align_defaults_from_base;

use super::merge_decl::{merge_frame_decl, merge_panel_decl, merge_world_decl};
use super::resolve::{resolve_frame_ref, resolve_panel_ref, resolve_world_ref};

pub(crate) fn resolve_panel_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<PanelDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if slot.get("kind").and_then(Value::as_str) != Some("panel") && expr.kind == RefKind::Panel
        {
            return resolve_panel_ref(app_root, &expr, scene_registry, diagnostics, target_file);
        }
    }

    if slot.get("kind").and_then(Value::as_str) != Some("panel") {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_frame_panel_slot".to_string(),
            message: "frame.panels entries must be panel(...), panel_ref(...), or panel(base=panel_ref(...))"
                .to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }

    if let Some(base_value) = slot.get("base").filter(|value| !value.is_null()) {
        let Some(expr) = decode_ref_value(base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_panel_base_ref_kind".to_string(),
                message: "panel(base=...) must be a panel_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_panel) =
            resolve_panel_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        let mut merged = merge_panel_decl(base_panel.clone(), slot).ok()?;
        seed_metric_slot_vertical_align_defaults_from_base(&base_panel, &mut merged, slot);
        merged.blocks = normalize_ui_nodes(
            app_root,
            &merged.blocks,
            scene_registry,
            diagnostics,
            target_file,
        );
        return Some(merged);
    }

    let mut panel: PanelDecl = match serde_json::from_value::<PanelDecl>(slot.clone()) {
        Ok(panel) => panel,
        Err(error) => {
            let message = error.to_string();
            let panel_id = slot
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<anonymous>");
            if message.contains("panel_ref_embed_removed") {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "panel_ref_embed_removed".to_string(),
                    message,
                    source_path: Some(target_file.to_string()),
                });
            } else {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_panel_decl".to_string(),
                    message: format!("panel `{panel_id}` failed to compile: {message}"),
                    source_path: Some(target_file.to_string()),
                });
            }
            return None;
        }
    };
    panel.blocks = normalize_ui_nodes(
        app_root,
        &panel.blocks,
        scene_registry,
        diagnostics,
        target_file,
    );
    panel.base = None;
    Some(panel)
}

pub(crate) fn normalize_frame_decl(
    app_root: &Path,
    frame: FrameDecl,
    frame_value: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FrameDecl> {
    let base_value = frame_value
        .get("base")
        .cloned()
        .or(frame.base.clone())
        .filter(|value| !value.is_null());
    if let Some(base_value) = base_value {
        let Some(expr) = decode_ref_value(&base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_frame_base_ref_kind".to_string(),
                message: "frame(base=...) must be a frame_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_frame) =
            resolve_frame_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        return merge_frame_decl(base_frame, frame_value).ok();
    }
    Some(frame)
}

pub(crate) fn normalize_world_decl(
    app_root: &Path,
    world: WorldDecl,
    world_value: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<WorldDecl> {
    let base_value = world_value
        .get("base")
        .cloned()
        .or(world.base.clone())
        .filter(|value| !value.is_null());
    if let Some(base_value) = base_value {
        let Some(expr) = decode_ref_value(&base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_world_base_ref_kind".to_string(),
                message: "world(base=...) must be a world_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_world) =
            resolve_world_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        let mut merged = merge_world_decl(base_world, world_value).ok()?;
        merged.resources = normalize_resource_list(
            app_root,
            &merged.resources,
            scene_registry,
            diagnostics,
            target_file,
        );
        merged.datasets = normalize_resource_list(
            app_root,
            &merged.datasets,
            scene_registry,
            diagnostics,
            target_file,
        );
        merged.metrics = normalize_metric_list(&merged.metrics);
        merged.metric_packs = normalize_resource_list(
            app_root,
            &merged.metric_packs,
            scene_registry,
            diagnostics,
            target_file,
        );
        merged.entities = normalize_entity_list(
            app_root,
            &merged.entities,
            scene_registry,
            diagnostics,
            target_file,
        );
        return Some(merged);
    }
    let mut merged = world;
    merged.resources = normalize_resource_list(
        app_root,
        &merged.resources,
        scene_registry,
        diagnostics,
        target_file,
    );
    merged.datasets = normalize_resource_list(
        app_root,
        &merged.datasets,
        scene_registry,
        diagnostics,
        target_file,
    );
    merged.metrics = normalize_metric_list(&merged.metrics);
    merged.metric_packs = normalize_resource_list(
        app_root,
        &merged.metric_packs,
        scene_registry,
        diagnostics,
        target_file,
    );
    merged.entities = normalize_entity_list(
        app_root,
        &merged.entities,
        scene_registry,
        diagnostics,
        target_file,
    );
    merged.base = None;
    Some(merged)
}
