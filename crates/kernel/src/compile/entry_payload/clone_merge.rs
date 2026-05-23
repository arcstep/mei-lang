//! `decl(base = *_ref(...))` 克隆与字段级覆盖归一。

use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::{
    Diagnostic, EntityDecl, FlowDecl, FrameDecl, PanelDecl, ResourceDecl, SceneDecl, Severity,
    UiNodeDecl, WorldDecl,
};
use crate::typed_refs::{decode_ref_value, RefExpr, RefKind, SceneRegistry};

use super::super::load_external::{
    load_block_from_scene_file, load_entity_from_world_file, load_flow_from_file,
    load_frame_from_file, load_panel_from_scene_file, load_resource_from_world_file,
    load_scene_from_file, load_world_from_file,
};

pub(crate) fn deep_merge_json(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            let mut out = base_map.clone();
            for (key, value) in overlay_map {
                if let Some(existing) = out.get(key) {
                    if existing.is_object() && value.is_object() {
                        out.insert(key.clone(), deep_merge_json(existing, value));
                    } else {
                        out.insert(key.clone(), value.clone());
                    }
                } else {
                    out.insert(key.clone(), value.clone());
                }
            }
            Value::Object(out)
        }
        _ => overlay.clone(),
    }
}

fn value_has_key(value: &Value, key: &str) -> bool {
    value.as_object().is_some_and(|map| map.contains_key(key))
}

fn resolve_ref_path(
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

fn push_invalid_base_kind(
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    kind_label: &str,
    expected: RefKind,
    got: RefKind,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: format!("invalid_{kind_label}_base_ref_kind"),
        message: format!(
            "{kind_label}(base=...) requires `{expected:?}` ref, got `{got:?}` ref"
        ),
        source_path: Some(target_file.to_string()),
    });
}

pub(super) fn resolve_panel_ref(
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
    let Some(path) = resolve_ref_path(expr, scene_registry, diagnostics, target_file, "panel_base_not_resolved")
    else {
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
    if let Some(base_value) = panel_value
        .get("base")
        .filter(|value| !value.is_null())
    {
        let Some(base_expr) = decode_ref_value(base_value) else {
            return Some(panel);
        };
        let Some(base_panel) =
            resolve_panel_ref(app_root, &base_expr, scene_registry, diagnostics, target_file)
        else {
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

pub(super) fn resolve_frame_ref(
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
    let Some(path) = resolve_ref_path(expr, scene_registry, diagnostics, target_file, "frame_base_not_resolved")
    else {
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

fn resolve_world_ref(
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
    let Some(path) = resolve_ref_path(expr, scene_registry, diagnostics, target_file, "world_base_not_resolved")
    else {
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

fn resolve_flow_ref(
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
    let Some(path) = resolve_ref_path(expr, scene_registry, diagnostics, target_file, "flow_base_not_resolved")
    else {
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

fn resolve_scene_ref(
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
    let Some(path) = resolve_ref_path(expr, scene_registry, diagnostics, target_file, "scene_base_not_resolved")
    else {
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

fn resource_ref_kind(expr: &RefExpr) -> Option<RefKind> {
    match expr.kind {
        RefKind::Resource | RefKind::Dataset | RefKind::Metric => Some(expr.kind),
        _ => None,
    }
}

fn resolve_resource_ref(
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
    let Some(path) =
        resolve_ref_path(expr, scene_registry, diagnostics, target_file, "resource_base_not_resolved")
    else {
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

fn resolve_entity_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<EntityDecl> {
    if expr.kind != RefKind::Entity {
        push_invalid_base_kind(diagnostics, target_file, "entity", RefKind::Entity, expr.kind);
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
    let Some(path) =
        resolve_ref_path(expr, scene_registry, diagnostics, target_file, "entity_base_not_resolved")
    else {
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

fn resolve_component_ref(
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

pub(super) fn merge_panel_decl(base: PanelDecl, overlay_value: &Value) -> Result<PanelDecl> {
    let overlay: PanelDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        let id = overlay.id.trim();
        if !id.is_empty() {
            merged.id = id.to_string();
        }
    }
    if value_has_key(overlay_value, "title") {
        merged.title = overlay.title;
    }
    if value_has_key(overlay_value, "area") {
        merged.area = overlay.area;
    }
    if value_has_key(overlay_value, "layout") {
        merged.layout = overlay.layout;
    }
    if value_has_key(overlay_value, "blocks") {
        let overlay_has_blocks = !overlay.blocks.is_empty();
        if overlay_has_blocks {
            merged.blocks = overlay.blocks;
        }
    }
    if value_has_key(overlay_value, "props") {
        merged.props = deep_merge_json(&merged.props, &overlay.props);
    }
    if value_has_key(overlay_value, "head_props") {
        merged.head_props = deep_merge_json(&merged.head_props, &overlay.head_props);
    }
    if value_has_key(overlay_value, "body_props") {
        merged.body_props = deep_merge_json(&merged.body_props, &overlay.body_props);
    }
    merged.kind = "panel".to_string();
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_frame_decl(base: FrameDecl, overlay_value: &Value) -> Result<FrameDecl> {
    let overlay: FrameDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        merged.id = overlay.id;
    }
    if value_has_key(overlay_value, "title") {
        merged.title = overlay.title;
    }
    if value_has_key(overlay_value, "layout") {
        merged.layout = overlay.layout;
    }
    if value_has_key(overlay_value, "props") {
        merged.props = deep_merge_json(&merged.props, &overlay.props);
    }
    if value_has_key(overlay_value, "panels") {
        merged.panels = overlay.panels;
    }
    merged.kind = "frame".to_string();
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_world_decl(base: WorldDecl, overlay_value: &Value) -> Result<WorldDecl> {
    let overlay: WorldDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        merged.id = overlay.id;
    }
    if value_has_key(overlay_value, "topology") {
        merged.topology = overlay.topology;
    }
    if value_has_key(overlay_value, "resources") {
        merged.resources = overlay.resources;
    }
    if value_has_key(overlay_value, "datasets") {
        merged.datasets = overlay.datasets;
    }
    if value_has_key(overlay_value, "metrics") {
        merged.metrics = overlay.metrics;
    }
    if value_has_key(overlay_value, "metric_packs") {
        merged.metric_packs = overlay.metric_packs;
    }
    if value_has_key(overlay_value, "entities") {
        merged.entities = overlay.entities;
    }
    merged.kind = "world".to_string();
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_flow_decl(base: FlowDecl, overlay_value: &Value) -> Result<FlowDecl> {
    let overlay: FlowDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        merged.id = overlay.id;
    }
    if value_has_key(overlay_value, "start") {
        merged.start = overlay.start;
    }
    if value_has_key(overlay_value, "interactions") {
        merged.interactions = overlay.interactions;
    }
    if value_has_key(overlay_value, "timer") {
        merged.timer = overlay.timer;
    }
    if value_has_key(overlay_value, "subject_timers") {
        merged.subject_timers = overlay.subject_timers;
    }
    if value_has_key(overlay_value, "outcome") {
        merged.outcome = overlay.outcome;
    }
    merged.kind = "flow".to_string();
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_scene_decl(base: SceneDecl, overlay_value: &Value) -> Result<SceneDecl> {
    let overlay: SceneDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        let id = overlay.id.trim();
        if !id.is_empty() {
            merged.id = id.to_string();
        }
    }
    if value_has_key(overlay_value, "world") {
        merged.world = overlay.world;
    }
    if value_has_key(overlay_value, "flow") {
        merged.flow = overlay.flow;
    }
    if value_has_key(overlay_value, "frame") {
        merged.frame = overlay.frame;
    }
    if value_has_key(overlay_value, "profile") {
        merged.profile = overlay.profile;
    }
    if value_has_key(overlay_value, "theme") {
        merged.theme = overlay.theme;
    }
    if value_has_key(overlay_value, "summary") {
        merged.summary = overlay.summary;
    }
    if value_has_key(overlay_value, "goal") {
        merged.goal = overlay.goal;
    }
    if value_has_key(overlay_value, "state") {
        merged.state = overlay.state;
    }
    if value_has_key(overlay_value, "access_export") {
        merged.access_export = overlay.access_export;
    }
    merged.kind = "scene".to_string();
    Ok(merged)
}

pub(super) fn merge_resource_decl(base: ResourceDecl, overlay_value: &Value) -> Result<ResourceDecl> {
    let overlay: ResourceDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        let id = overlay.id.trim();
        if !id.is_empty() {
            merged.id = id.to_string();
        }
    }
    if value_has_key(overlay_value, "kind") {
        merged.kind = overlay.kind;
    }
    if value_has_key(overlay_value, "title") {
        merged.title = overlay.title;
    }
    if value_has_key(overlay_value, "purpose") {
        merged.purpose = overlay.purpose;
    }
    if value_has_key(overlay_value, "source") {
        merged.source = overlay.source;
    }
    if value_has_key(overlay_value, "content") {
        merged.content = overlay.content;
    }
    if value_has_key(overlay_value, "dataset") {
        merged.dataset = overlay.dataset;
    }
    if value_has_key(overlay_value, "metrics") {
        merged.metrics = overlay.metrics;
    }
    if value_has_key(overlay_value, "filters") {
        merged.filters = overlay.filters;
    }
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_entity_decl(base: EntityDecl, overlay_value: &Value) -> Result<EntityDecl> {
    let overlay: EntityDecl = serde_json::from_value(overlay_value.clone())?;
    let mut merged = base;
    if value_has_key(overlay_value, "id") {
        let id = overlay.id.trim();
        if !id.is_empty() {
            merged.id = id.to_string();
        }
    }
    if value_has_key(overlay_value, "kind") {
        merged.kind = overlay.kind;
    }
    if value_has_key(overlay_value, "label") {
        merged.label = overlay.label;
    }
    if value_has_key(overlay_value, "spawns") {
        merged.spawns = overlay.spawns;
    }
    if value_has_key(overlay_value, "status") {
        merged.status = overlay.status;
    }
    if value_has_key(overlay_value, "flags") {
        merged.flags = deep_merge_json(&merged.flags, &overlay.flags);
    }
    merged.base = None;
    Ok(merged)
}

pub(super) fn merge_block_value(base: Value, overlay_value: &Value) -> Value {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    let overlay = overlay_value.as_object().cloned().unwrap_or_default();
    for (key, value) in overlay {
        if key == "base" {
            continue;
        }
        if key == "props" {
            let base_props = merged.get("props").cloned().unwrap_or(Value::Object(Default::default()));
            merged.insert(key, deep_merge_json(&base_props, &value));
            continue;
        }
        if key == "component" {
            if let (Some(base_component), Some(overlay_component)) =
                (merged.get("component"), Some(&value))
            {
                if base_component.is_object() && overlay_component.is_object() {
                    merged.insert(
                        key,
                        deep_merge_json(base_component, overlay_component),
                    );
                    continue;
                }
            }
        }
        merged.insert(key, value);
    }
    merged.remove("base");
    Value::Object(merged)
}

pub(super) fn resolve_panel_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<PanelDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if slot.get("kind").and_then(Value::as_str) != Some("panel") && expr.kind == RefKind::Panel {
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
        let mut merged = merge_panel_decl(base_panel, slot).ok()?;
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

pub(super) fn normalize_frame_decl(
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

pub(super) fn normalize_world_decl(
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

fn normalize_metric_list(items: &[Value]) -> Vec<Value> {
    let mut merged = Vec::<Value>::new();
    for item in items {
        let key = item
            .get("key")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
        if let Some(key) = key {
            if let Some(existing) = merged.iter_mut().find(|current| {
                current
                    .get("key")
                    .or_else(|| current.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == Some(key.as_str())
            }) {
                *existing = item.clone();
                continue;
            }
        }
        merged.push(item.clone());
    }
    merged
}

pub(super) fn normalize_flow_decl(
    app_root: &Path,
    flow: FlowDecl,
    flow_value: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FlowDecl> {
    let base_value = flow_value
        .get("base")
        .cloned()
        .or(flow.base.clone())
        .filter(|value| !value.is_null());
    if let Some(base_value) = base_value {
        let Some(expr) = decode_ref_value(&base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_flow_base_ref_kind".to_string(),
                message: "flow(base=...) must be a flow_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_flow) =
            resolve_flow_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        return merge_flow_decl(base_flow, flow_value).ok();
    }
    let mut merged = flow;
    merged.base = None;
    Some(merged)
}

pub(crate) fn normalize_scene_value(
    app_root: &Path,
    scene_value: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Value> {
    let base_value = scene_value.get("base").filter(|value| !value.is_null())?;
    let Some(expr) = decode_ref_value(base_value) else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_base_ref_kind".to_string(),
            message: "scene(base=...) must be a scene_ref(...) value".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    };
    let Some(base_scene) =
        resolve_scene_ref(app_root, &expr, scene_registry, diagnostics, target_file)
    else {
        return None;
    };
    let merged = merge_scene_decl(base_scene, scene_value).ok()?;
    serde_json::to_value(merged).ok()
}

pub(super) fn resolve_resource_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<ResourceDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if resource_ref_kind(&expr).is_some() && !slot.as_object().is_some_and(|m| m.contains_key("id")) {
            return resolve_resource_ref(
                app_root,
                &expr,
                scene_registry,
                diagnostics,
                target_file,
                None,
            );
        }
    }
    if let Some(base_value) = slot.get("base").filter(|value| !value.is_null()) {
        let Some(expr) = decode_ref_value(base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_resource_base_ref_kind".to_string(),
                message: "resource/dataset/metric(base=...) must be a matching *_ref(...) value"
                    .to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let overlay_kind = slot.get("kind").and_then(Value::as_str);
        let Some(base_resource) = resolve_resource_ref(
            app_root,
            &expr,
            scene_registry,
            diagnostics,
            target_file,
            overlay_kind,
        ) else {
            return None;
        };
        return merge_resource_decl(base_resource, slot).ok();
    }
    serde_json::from_value::<ResourceDecl>(slot.clone()).ok()
}

pub(super) fn resolve_entity_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<EntityDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if expr.kind == RefKind::Entity && !slot.as_object().is_some_and(|m| m.contains_key("kind")) {
            return resolve_entity_ref(app_root, &expr, scene_registry, diagnostics, target_file);
        }
    }
    if let Some(base_value) = slot.get("base").filter(|value| !value.is_null()) {
        let Some(expr) = decode_ref_value(base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_entity_base_ref_kind".to_string(),
                message: "entity(base=...) must be an entity_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_entity) =
            resolve_entity_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        return merge_entity_decl(base_entity, slot).ok();
    }
    serde_json::from_value::<EntityDecl>(slot.clone()).ok()
}

pub(super) fn resolve_block_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Value> {
    if slot.get("kind").and_then(Value::as_str) != Some("block")
        && !slot.as_object().is_some_and(|m| m.contains_key("use_key"))
    {
        return Some(slot.clone());
    }
    if let Some(base_value) = slot.get("base").filter(|value| !value.is_null()) {
        let Some(expr) = decode_ref_value(base_value) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_component_base_ref_kind".to_string(),
                message: "component(base=...) must be a component_ref(...) value".to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        let Some(base_block) =
            resolve_component_ref(app_root, &expr, scene_registry, diagnostics, target_file)
        else {
            return None;
        };
        return Some(merge_block_value(base_block, slot));
    }
    let mut out = slot.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.remove("base");
    }
    Some(out)
}

fn normalize_resource_list(
    app_root: &Path,
    resources: &[ResourceDecl],
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Vec<ResourceDecl> {
    resources
        .iter()
        .filter_map(|resource| {
            let value = serde_json::to_value(resource).ok()?;
            resolve_resource_slot(app_root, &value, scene_registry, diagnostics, target_file)
        })
        .collect()
}

fn normalize_entity_list(
    app_root: &Path,
    entities: &[EntityDecl],
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Vec<EntityDecl> {
    entities
        .iter()
        .filter_map(|entity| {
            let value = serde_json::to_value(entity).ok()?;
            resolve_entity_slot(app_root, &value, scene_registry, diagnostics, target_file)
        })
        .collect()
}

pub(super) fn normalize_ui_nodes(
    app_root: &Path,
    nodes: &[UiNodeDecl],
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Vec<UiNodeDecl> {
    nodes
        .iter()
        .filter_map(|node| normalize_ui_node(app_root, node, scene_registry, diagnostics, target_file))
        .collect()
}

fn normalize_ui_node(
    app_root: &Path,
    node: &UiNodeDecl,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<UiNodeDecl> {
    match node {
        UiNodeDecl::Panel(panel) => {
            let value = serde_json::to_value(panel.clone()).ok()?;
            if panel.base.is_some() || value.get("base").is_some() {
                return resolve_panel_slot(app_root, &value, scene_registry, diagnostics, target_file)
                    .map(UiNodeDecl::Panel);
            }
            let mut panel = panel.clone();
            panel.blocks = normalize_ui_nodes(
                app_root,
                &panel.blocks,
                scene_registry,
                diagnostics,
                target_file,
            );
            panel.base = None;
            Some(UiNodeDecl::Panel(panel))
        }
        UiNodeDecl::Block(block) => {
            if block.base.is_none() {
                return Some(node.clone());
            }
            let value = serde_json::to_value(node).ok()?;
            let normalized = resolve_block_slot(
                app_root,
                &value,
                scene_registry,
                diagnostics,
                target_file,
            )?;
            deserialize_ui_node_value(normalized).ok()
        }
        UiNodeDecl::PanelRefEmbed(embed) => {
            let (code, message) = match embed.compat_source.as_deref() {
                Some("panel_capsule_ref") => (
                    "deprecated_panel_capsule_ref",
                    "panel_capsule_ref block embed is deprecated; use frame.panels panel_ref(...) or panel(base=panel_ref(...))",
                ),
                Some("frame_ref") => (
                    "deprecated_frame_ref_block_embed",
                    "frame_ref block embed is deprecated; use scene(frame=frame_ref(...)) or frame(base=frame_ref(...))",
                ),
                _ => (
                    "panel_ref_embed_removed",
                    "panel_ref only references external panels in frame.panels; \
                     block embed with `area` is no longer supported",
                ),
            };
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: code.to_string(),
                message: message.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

fn deserialize_ui_node_value(value: Value) -> Result<UiNodeDecl, String> {
    crate::model::deserialize_ui_node_value(value)
}

/// 递归收集 `*_ref` 的 `scene_file`，用于 world 资源合并。
pub(super) fn collect_ref_scene_files(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    if let Some(expr) = decode_ref_value(value) {
        if let Some(path) = expr
            .locator
            .scene_file
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            out.insert(path.to_string());
        }
    }
    if let Some(base) = value.get("base") {
        collect_ref_scene_files(base, out);
    }
    match value {
        Value::Array(items) => {
            for item in items {
                collect_ref_scene_files(item, out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                if matches!(key.as_str(), "blocks" | "panels" | "resources" | "datasets" | "metrics" | "metric_packs" | "entities") {
                    collect_ref_scene_files(item, out);
                }
            }
        }
        _ => {}
    }
}
