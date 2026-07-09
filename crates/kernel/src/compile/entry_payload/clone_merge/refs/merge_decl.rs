use anyhow::Result;
use serde_json::Value;

use crate::model::{
    EntityDecl, FlowDecl, FrameDecl, UiNodeDecl, ResourceDecl, SceneDecl, WorldDecl,
};

use super::super::merge::{deep_merge_json, value_has_key};

pub(crate) fn merge_panel_decl(base: UiNodeDecl, overlay_value: &Value) -> Result<UiNodeDecl> {
    let overlay: UiNodeDecl = serde_json::from_value(overlay_value.clone())?;
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
    // metric_card(base=...) lowers to panel(...) without layout; do not wipe template grid.
    if value_has_key(overlay_value, "layout") && overlay.layout.is_some() {
        merged.layout = overlay.layout;
    }
    if value_has_key(overlay_value, "blocks") {
        let overlay_has_blocks = !overlay.blocks.is_empty();
        if overlay_has_blocks {
            merged.blocks = overlay.blocks;
        }
    }
    if value_has_key(overlay_value, "slot") {
        merged.slot = overlay.slot;
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

pub(crate) fn merge_frame_decl(base: FrameDecl, overlay_value: &Value) -> Result<FrameDecl> {
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

pub(crate) fn merge_world_decl(base: WorldDecl, overlay_value: &Value) -> Result<WorldDecl> {
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

pub(crate) fn merge_flow_decl(base: FlowDecl, overlay_value: &Value) -> Result<FlowDecl> {
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

pub(crate) fn merge_scene_decl(base: SceneDecl, overlay_value: &Value) -> Result<SceneDecl> {
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
    if value_has_key(overlay_value, "shared") {
        merged.shared = deep_merge_json(&merged.shared, &overlay.shared);
    }
    if value_has_key(overlay_value, "local_nav") {
        merged.local_nav = deep_merge_json(&merged.local_nav, &overlay.local_nav);
    }
    if value_has_key(overlay_value, "params") {
        merged.params = deep_merge_json(&merged.params, &overlay.params);
    }
    if value_has_key(overlay_value, "bindings") {
        merged.bindings = deep_merge_json(&merged.bindings, &overlay.bindings);
    }
    if value_has_key(overlay_value, "examples") {
        merged.examples = overlay.examples;
    }
    if value_has_key(overlay_value, "access_export") {
        merged.access_export = overlay.access_export;
    }
    merged.kind = "scene".to_string();
    Ok(merged)
}

pub(crate) fn merge_resource_decl(
    base: ResourceDecl,
    overlay_value: &Value,
) -> Result<ResourceDecl> {
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

pub(crate) fn merge_entity_decl(base: EntityDecl, overlay_value: &Value) -> Result<EntityDecl> {
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

pub(crate) fn merge_block_value(base: Value, overlay_value: &Value) -> Value {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    let overlay = overlay_value.as_object().cloned().unwrap_or_default();
    for (key, value) in overlay {
        if key == "base" {
            continue;
        }
        if key == "props" {
            let base_props = merged
                .get("props")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            merged.insert(key, deep_merge_json(&base_props, &value));
            continue;
        }
        if key == "component" {
            if let (Some(base_component), Some(overlay_component)) =
                (merged.get("component"), Some(&value))
            {
                if base_component.is_object() && overlay_component.is_object() {
                    merged.insert(key, deep_merge_json(base_component, overlay_component));
                    continue;
                }
            }
        }
        merged.insert(key, value);
    }
    merged.remove("base");
    Value::Object(merged)
}
