use std::path::Path;

use serde_json::Value;

use crate::compile::load_external::{
    load_block_from_scene_file, load_entity_from_world_file, load_flow_from_file,
    load_frame_from_file, load_panel_from_scene_file, load_resource_from_world_file,
    load_scene_from_file, load_world_from_file,
};
use crate::model::{
    Diagnostic, EntityDecl, FlowDecl, FrameDecl, PanelDecl, ResourceDecl, SceneDecl, Severity,
    WorldDecl,
};
use crate::typed_refs::{decode_ref_value, RefExpr, RefKind, SceneRegistry};

use super::super::normalize::normalize_ui_nodes;
use super::merge_decl::merge_panel_decl;

pub(crate) fn resolve_ref_path(
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    not_resolved_code: &str,
) -> Option<String> {
    if let Some(path) = expr
        .locator
        .scene_file
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Some(path.to_string());
    }
    match scene_registry.resolve_target(&expr.locator) {
        Ok((_, path)) => Some(path),
        Err(message) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: not_resolved_code.to_string(),
                message,
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn push_invalid_base_kind(
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    kind_label: &str,
    expected: RefKind,
    got: RefKind,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: format!("invalid_{kind_label}_base_ref_kind"),
        message: format!("{kind_label}(base=...) requires `{expected:?}` ref, got `{got:?}` ref"),
        source_path: Some(target_file.to_string()),
    });
}

pub(crate) fn resolve_panel_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<PanelDecl> {
    if expr.kind != RefKind::Panel {
        push_invalid_base_kind(diagnostics, target_file, "panel", RefKind::Panel, expr.kind);
        return None;
    }
    let panel_id = expr.id.as_deref().unwrap_or_default().trim();
    if panel_id.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_panel_ref".to_string(),
            message: "panel_ref(...) requires panel id".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "panel_base_not_resolved",
    ) else {
        return None;
    };
    let panel = match load_panel_from_scene_file(app_root, path.as_str(), panel_id) {
        Ok(panel) => panel,
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "panel_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
    };
    let Some(panel_value) = serde_json::to_value(&panel).ok() else {
        return Some(panel);
    };
    if let Some(base_value) = panel_value.get("base").filter(|value| !value.is_null()) {
        let Some(base_expr) = decode_ref_value(base_value) else {
            return Some(panel);
        };
        let Some(base_panel) = resolve_panel_ref(
            app_root,
            &base_expr,
            scene_registry,
            diagnostics,
            target_file,
        ) else {
            return None;
        };
        let mut overlay = panel_value.clone();
        if let Some(obj) = overlay.as_object_mut() {
            if obj
                .get("blocks")
                .and_then(Value::as_array)
                .is_some_and(|blocks| blocks.is_empty())
            {
                obj.remove("blocks");
            }
            obj.remove("base");
        }
        let mut merged = merge_panel_decl(base_panel, &overlay).ok()?;
        merged.blocks = normalize_ui_nodes(
            app_root,
            &merged.blocks,
            scene_registry,
            diagnostics,
            target_file,
        );
        merged.base = None;
        return Some(merged);
    }
    Some(panel)
}

pub(crate) fn resolve_frame_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FrameDecl> {
    if expr.kind != RefKind::Frame {
        push_invalid_base_kind(diagnostics, target_file, "frame", RefKind::Frame, expr.kind);
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "frame_base_not_resolved",
    ) else {
        return None;
    };
    let frame_id = expr
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    match load_frame_from_file(app_root, path.as_str(), frame_id) {
        Ok(frame) => Some(frame),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "frame_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resolve_world_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<WorldDecl> {
    if expr.kind != RefKind::World {
        push_invalid_base_kind(diagnostics, target_file, "world", RefKind::World, expr.kind);
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "world_base_not_resolved",
    ) else {
        return None;
    };
    let world_id = expr
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    match load_world_from_file(app_root, path.as_str(), world_id) {
        Ok(world) => Some(world),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "world_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resolve_flow_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FlowDecl> {
    if expr.kind != RefKind::Flow {
        push_invalid_base_kind(diagnostics, target_file, "flow", RefKind::Flow, expr.kind);
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "flow_base_not_resolved",
    ) else {
        return None;
    };
    let flow_id = expr
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    match load_flow_from_file(app_root, path.as_str(), flow_id) {
        Ok(flow) => Some(flow),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "flow_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resolve_scene_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<SceneDecl> {
    if expr.kind != RefKind::Scene {
        push_invalid_base_kind(diagnostics, target_file, "scene", RefKind::Scene, expr.kind);
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "scene_base_not_resolved",
    ) else {
        return None;
    };
    let scene_id = expr
        .id
        .as_deref()
        .or(expr.locator.scene_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty());
    match load_scene_from_file(app_root, path.as_str(), scene_id) {
        Ok(scene) => Some(scene),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "scene_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resource_ref_kind(expr: &RefExpr) -> Option<RefKind> {
    match expr.kind {
        RefKind::Resource | RefKind::Dataset | RefKind::Metric => Some(expr.kind),
        _ => None,
    }
}

pub(crate) fn resolve_resource_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    overlay_kind: Option<&str>,
) -> Option<ResourceDecl> {
    let Some(ref_kind) = resource_ref_kind(expr) else {
        push_invalid_base_kind(
            diagnostics,
            target_file,
            "resource",
            RefKind::Resource,
            expr.kind,
        );
        return None;
    };
    let resource_id = expr.id.as_deref().unwrap_or_default().trim();
    if resource_id.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_resource_ref".to_string(),
            message: "resource_ref/dataset_ref/metric_ref requires id".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "resource_base_not_resolved",
    ) else {
        return None;
    };
    let expected_kind = overlay_kind.or_else(|| match ref_kind {
        RefKind::Dataset => Some("dataset"),
        RefKind::Metric => Some("metric_pack"),
        _ => None,
    });
    match load_resource_from_world_file(app_root, path.as_str(), resource_id, expected_kind) {
        Ok(resource) => Some(resource),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "resource_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resolve_entity_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<EntityDecl> {
    if expr.kind != RefKind::Entity {
        push_invalid_base_kind(
            diagnostics,
            target_file,
            "entity",
            RefKind::Entity,
            expr.kind,
        );
        return None;
    }
    let entity_id = expr.id.as_deref().unwrap_or_default().trim();
    if entity_id.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_entity_ref".to_string(),
            message: "entity_ref(...) requires id".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "entity_base_not_resolved",
    ) else {
        return None;
    };
    match load_entity_from_world_file(app_root, path.as_str(), entity_id) {
        Ok(entity) => Some(entity),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "entity_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn resolve_component_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Value> {
    if expr.kind != RefKind::Component {
        push_invalid_base_kind(
            diagnostics,
            target_file,
            "component",
            RefKind::Component,
            expr.kind,
        );
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "component_base_not_resolved",
    ) else {
        return None;
    };
    let block_id = expr
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let use_key = expr
        .use_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    match load_block_from_scene_file(app_root, path.as_str(), block_id, use_key) {
        Ok(block) => Some(block),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "component_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}
