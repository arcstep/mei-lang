//! `decl(base = *_ref(...))` 克隆与字段级覆盖归一。

use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::{
    Diagnostic, EntityDecl, FlowDecl, ResourceDecl, Severity,
    UiNodeDecl,
};
use crate::typed_refs::{decode_ref_value, RefKind, SceneRegistry};



use super::refs::{
    merge_block_value, merge_entity_decl, merge_flow_decl,
    merge_resource_decl, merge_scene_decl, resolve_component_ref,
    resolve_entity_ref, resolve_flow_ref, resolve_panel_slot,
    resolve_resource_ref, resolve_scene_ref, resource_ref_kind,
};

pub(super) fn normalize_metric_list(items: &[Value]) -> Vec<Value> {
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

pub(crate) fn normalize_flow_decl(
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

pub(crate) fn resolve_resource_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<ResourceDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if resource_ref_kind(&expr).is_some()
            && !slot.as_object().is_some_and(|m| m.contains_key("id"))
        {
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

pub(crate) fn resolve_entity_slot(
    app_root: &Path,
    slot: &Value,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<EntityDecl> {
    if let Some(expr) = decode_ref_value(slot) {
        if expr.kind == RefKind::Entity && !slot.as_object().is_some_and(|m| m.contains_key("kind"))
        {
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

pub(super) fn normalize_resource_list(
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

pub(super) fn normalize_entity_list(
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
        .filter_map(|node| {
            normalize_ui_node(app_root, node, scene_registry, diagnostics, target_file)
        })
        .collect()
}

pub(super) fn normalize_ui_node(
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
                return resolve_panel_slot(
                    app_root,
                    &value,
                    scene_registry,
                    diagnostics,
                    target_file,
                )
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
            let normalized =
                resolve_block_slot(app_root, &value, scene_registry, diagnostics, target_file)?;
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

pub(super) fn deserialize_ui_node_value(value: Value) -> Result<UiNodeDecl, String> {
    crate::model::deserialize_ui_node_value(value)
}

/// 递归收集 `*_ref` 的 `scene_file`，用于 world 资源合并。
pub(crate) fn collect_ref_scene_files(value: &Value, out: &mut std::collections::BTreeSet<String>) {
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
                if matches!(
                    key.as_str(),
                    "blocks"
                        | "panels"
                        | "resources"
                        | "datasets"
                        | "metrics"
                        | "metric_packs"
                        | "entities"
                ) {
                    collect_ref_scene_files(item, out);
                }
            }
        }
        _ => {}
    }
}
