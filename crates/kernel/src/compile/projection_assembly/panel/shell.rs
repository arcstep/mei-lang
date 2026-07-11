use super::shell_zones::layout_decl_to_value;
use super::{
    collect_scene_shell_zones, collect_top_level_layout_areas, dedupe_shell_zones_by_id,
    infer_scene_shell_layout_mode,
};

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::model::SceneContract;

pub(crate) fn scene_shell_contract_from_scene_contract(
    contract: &SceneContract,
) -> Option<Map<String, Value>> {
    let mut zones = Vec::new();
    collect_scene_shell_zones(&contract.panels, "", &mut zones);
    let layout = contract
        .frame
        .as_ref()
        .and_then(|frame| frame.layout.as_ref())
        .map(layout_decl_to_value);
    if zones.is_empty() && layout.is_none() {
        return None;
    }
    if let Some(ref layout) = layout {
        retain_shell_zones_matching_layout(layout, &mut zones);
    }
    let top_level_areas = layout
        .as_ref()
        .map(collect_top_level_layout_areas)
        .unwrap_or_default();
    dedupe_shell_zones_by_id(&mut zones, &top_level_areas);
    let layout_mode = infer_scene_shell_layout_mode(&zones);
    let mut payload = Map::new();
    payload.insert(
        "__kind".to_string(),
        Value::String("scene_shell_contract".to_string()),
    );
    if !layout_mode.is_empty() {
        payload.insert("layout_mode".to_string(), Value::String(layout_mode));
    }
    if let Some(layout) = layout {
        payload.insert("layout".to_string(), layout);
    }
    payload.insert("zones".to_string(), Value::Array(zones));
    if let Some(overlay_size) = contract
        .scene
        .local_nav
        .get("overlay_size")
        .or_else(|| contract.scene.local_nav.get("overlaySize"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload.insert(
            "overlay_size".to_string(),
            Value::String(overlay_size.to_string()),
        );
    }
    Some(payload)
}

fn retain_shell_zones_matching_layout(layout: &Value, zones: &mut Vec<Value>) {
    let Some(areas) = layout.get("areas").and_then(Value::as_array) else {
        return;
    };
    let mut allowed = BTreeSet::new();
    for row in areas {
        let Some(cells) = row.as_array() else {
            continue;
        };
        for cell in cells {
            let Some(area) = cell
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if area != "." {
                allowed.insert(area.to_string());
            }
        }
    }
    if allowed.is_empty() {
        return;
    }
    let mut kept_ids = BTreeSet::new();
    for zone in zones.iter() {
        let Some(map) = zone.as_object() else {
            continue;
        };
        let Some(area) = map
            .get("area")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if allowed.contains(area) {
            if let Some(id) = map
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            {
                kept_ids.insert(id.to_string());
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for zone in zones.iter() {
            let Some(map) = zone.as_object() else {
                continue;
            };
            let Some(container_id) = map
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && kept_ids.contains(*id))
            else {
                continue;
            };
            if map.get("role").and_then(Value::as_str) != Some("container") {
                continue;
            }
            let Some(nested_layout) = map.get("layout") else {
                continue;
            };
            let nested_areas = collect_top_level_layout_areas(nested_layout);
            for candidate in zones.iter() {
                let Some(candidate_map) = candidate.as_object() else {
                    continue;
                };
                let Some(area) = candidate_map
                    .get("area")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if !nested_areas.contains(area) {
                    continue;
                }
                if let Some(id) = candidate_map
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                {
                    if kept_ids.insert(id.to_string()) {
                        changed = true;
                    }
                }
            }
            let _ = container_id;
        }
    }
    changed = true;
    while changed {
        changed = false;
        for zone in zones.iter() {
            let Some(map) = zone.as_object() else {
                continue;
            };
            let Some(id) = map
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            if kept_ids.contains(id) {
                continue;
            }
            let Some(parent) = map
                .get("parent")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if kept_ids.contains(parent) && kept_ids.insert(id.to_string()) {
                changed = true;
            }
        }
    }
    zones.retain(|zone| {
        zone.as_object()
            .and_then(|map| map.get("id"))
            .and_then(Value::as_str)
            .is_some_and(|id| kept_ids.contains(id))
    });
}
