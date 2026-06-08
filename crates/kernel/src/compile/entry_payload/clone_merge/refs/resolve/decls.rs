use std::path::Path;

use serde_json::Value;

use crate::compile::load_external::{
    load_block_from_scene_file, load_entity_from_world_file, load_flow_from_file,
    load_frame_from_file, load_resource_from_world_file, load_scene_from_file,
    load_world_from_file,
};
use crate::model::{
    Diagnostic, EntityDecl, FlowDecl, FrameDecl, ResourceDecl, SceneDecl, Severity, WorldDecl,
};
use crate::typed_refs::{RefExpr, RefKind, SceneRegistry};

use super::path::{push_invalid_base_kind, resolve_ref_path};

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
