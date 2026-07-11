use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::model::{LayoutDecl, PanelSlotDecl, UiNodeDecl, UiTreeNode};

pub(super) fn collect_top_level_layout_areas(layout: &Value) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();
    let Some(areas) = layout.get("areas").and_then(Value::as_array) else {
        return allowed;
    };
    for row in areas {
        let Some(cells) = row.as_array() else {
            continue;
        };
        for cell in cells {
            let Some(area) = cell
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != ".")
            else {
                continue;
            };
            allowed.insert(area.to_string());
        }
    }
    allowed
}

fn shell_zone_dedupe_rank(
    zone: &Map<String, Value>,
    top_level_areas: &BTreeSet<String>,
) -> (i32, i32) {
    let parent = zone
        .get("parent")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let area = zone
        .get("area")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let area_in_top = !area.is_empty() && top_level_areas.contains(area);
    let is_nested = !parent.is_empty();
    let primary = if area_in_top {
        if is_nested {
            0
        } else {
            1
        }
    } else if is_nested {
        1
    } else {
        0
    };
    (primary, if is_nested { 1 } else { 0 })
}

/// Cockpit profile wraps board frame in nested panels; retain may keep duplicate zone ids
/// (e.g. chart under `left` and chart at frame root). Prefer the variant that matches
/// whether the zone `area` is a top-level frame cell or nested-only.
pub(super) fn dedupe_shell_zones_by_id(zones: &mut Vec<Value>, top_level_areas: &BTreeSet<String>) {
    let mut best_by_id: BTreeMap<String, Value> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for zone in zones.drain(..) {
        let Some(map) = zone.as_object() else {
            continue;
        };
        let Some(id) = map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let replace = match best_by_id.get(id) {
            None => true,
            Some(existing) => {
                let existing_map = existing.as_object().expect("zone object");
                shell_zone_dedupe_rank(map, top_level_areas)
                    > shell_zone_dedupe_rank(existing_map, top_level_areas)
            }
        };
        if replace {
            if !best_by_id.contains_key(id) {
                order.push(id.to_string());
            }
            best_by_id.insert(id.to_string(), zone);
        }
    }
    zones.extend(order.into_iter().filter_map(|id| best_by_id.remove(&id)));
}

pub(super) fn infer_scene_shell_layout_mode(zones: &[Value]) -> String {
    let mut has_tab_bar = false;
    let mut has_tab_content = false;
    let mut has_row_preview = false;
    let mut has_filter = false;
    let mut has_slots = false;
    let mut has_analytics_content = false;
    for zone in zones {
        let Some(role) = zone
            .as_object()
            .and_then(|map| map.get("role"))
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            continue;
        };
        match role {
            "tab_bar" => has_tab_bar = true,
            "tab_content" => has_tab_content = true,
            "row_preview" => has_row_preview = true,
            "filter" => has_filter = true,
            "slots" => has_slots = true,
            _ => {}
        }
        if zone_implies_analytics_content(zone) {
            has_analytics_content = true;
        }
    }
    if has_tab_bar && has_tab_content {
        return "generic_tabs".to_string();
    }
    if has_row_preview {
        return "list_preview".to_string();
    }
    if has_filter && (has_slots || has_analytics_content) {
        return "analytics".to_string();
    }
    String::new()
}

fn zone_implies_analytics_content(zone: &Value) -> bool {
    let Some(map) = zone.as_object() else {
        return false;
    };
    if map.get("role").and_then(Value::as_str) == Some("slots") {
        return true;
    }
    if map
        .get("accepts")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|value| matches!(value.as_str(), Some("chart") | Some("data_table")))
        })
    {
        return true;
    }
    map.get("layout")
        .and_then(|layout| layout.get("areas"))
        .and_then(Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.as_array().is_some_and(|cells| {
                    cells
                        .iter()
                        .any(|cell| matches!(cell.as_str(), Some("chart") | Some("detail")))
                })
            })
        })
}

pub(super) fn collect_scene_shell_zones(panels: &[UiNodeDecl], parent: &str, out: &mut Vec<Value>) {
    for panel in panels {
        if let Some(zone) = panel_zone_to_value(panel, parent) {
            out.push(Value::Object(zone));
        }
        let child_panels = panel
            .blocks
            .iter()
            .filter_map(|node| match node {
                UiTreeNode::Panel(child) => Some(child.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let next_parent = panel.id.as_str();
        collect_scene_shell_zones(&child_panels, next_parent, out);
    }
}

fn panel_zone_to_value(panel: &UiNodeDecl, parent: &str) -> Option<Map<String, Value>> {
    let slot_map = panel_slot_as_map(panel)?;
    let role = slot_map
        .get("kind")
        .or_else(|| slot_map.get("role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut zone = Map::new();
    zone.insert("id".to_string(), Value::String(panel.id.clone()));
    zone.insert("role".to_string(), Value::String(role.to_string()));
    if let Some(area) = panel
        .area
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert("area".to_string(), Value::String(area.to_string()));
    }
    if !parent.trim().is_empty() {
        zone.insert(
            "parent".to_string(),
            Value::String(parent.trim().to_string()),
        );
    }
    if let Some(source) = slot_map
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert("source".to_string(), Value::String(source.to_string()));
    }
    if let Some(selection_source) = slot_map
        .get("selection_from")
        .or_else(|| slot_map.get("selectionFrom"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        zone.insert(
            "selection_source".to_string(),
            Value::String(selection_source.to_string()),
        );
    }
    if let Some(required) = slot_map.get("required") {
        zone.insert("required".to_string(), required.clone());
    }
    if let Some(max) = slot_map.get("max") {
        zone.insert("max".to_string(), max.clone());
    }
    if let Some(accepts) = slot_map.get("accepts").and_then(Value::as_array) {
        zone.insert("accepts".to_string(), Value::Array(accepts.clone()));
    }
    if let Some(layout) = panel.layout.as_ref() {
        zone.insert("layout".to_string(), layout_decl_to_value(layout));
    }
    Some(zone)
}

fn panel_slot_as_map(panel: &UiNodeDecl) -> Option<Map<String, Value>> {
    if let Some(slot) = panel
        .slot
        .as_ref()
        .filter(|slot| panel_slot_decl_is_meaningful(slot))
    {
        return Some(panel_slot_decl_to_map(slot));
    }
    let props = panel.props.as_object()?;
    if let Some(slot) = props.get("__mei_panel_slot").and_then(Value::as_object) {
        return Some(slot.clone());
    }
    let role = props
        .get("projection_role")
        .or_else(|| props.get("zone_role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut migrated = Map::new();
    migrated.insert("kind".to_string(), Value::String(role.to_string()));
    if let Some(source) = props
        .get("projection_source")
        .or_else(|| props.get("source"))
        .filter(|value| !value.is_null())
    {
        migrated.insert("source".to_string(), source.clone());
    }
    if let Some(selection) = props
        .get("selection_source")
        .or_else(|| props.get("selectionSource"))
        .filter(|value| !value.is_null())
    {
        migrated.insert("selection_from".to_string(), selection.clone());
    }
    if let Some(required) = props.get("projection_required") {
        migrated.insert("required".to_string(), required.clone());
    }
    if let Some(max) = props.get("projection_max") {
        migrated.insert("max".to_string(), max.clone());
    }
    if let Some(accepts) = props.get("projection_accepts").and_then(Value::as_array) {
        migrated.insert("accepts".to_string(), Value::Array(accepts.clone()));
    }
    Some(migrated)
}

fn panel_slot_decl_is_meaningful(slot: &PanelSlotDecl) -> bool {
    slot.kind
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn panel_slot_decl_to_map(slot: &PanelSlotDecl) -> Map<String, Value> {
    let value = serde_json::to_value(slot).unwrap_or(Value::Null);
    value.as_object().cloned().unwrap_or_default()
}

pub(super) fn layout_decl_to_value(layout: &LayoutDecl) -> Value {
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(layout.layout_type.clone()),
    );
    if let Some(direction) = layout.direction.as_ref() {
        out.insert("direction".to_string(), Value::String(direction.clone()));
    }
    if let Some(columns) = layout.columns.as_ref() {
        out.insert(
            "columns".to_string(),
            Value::Array(columns.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(rows) = layout.rows.as_ref() {
        out.insert(
            "rows".to_string(),
            Value::Array(rows.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(areas) = layout.areas.as_ref() {
        out.insert(
            "areas".to_string(),
            Value::Array(
                areas
                    .iter()
                    .map(|row| Value::Array(row.iter().cloned().map(Value::String).collect()))
                    .collect(),
            ),
        );
    }
    if let Some(gap) = layout.gap.as_ref() {
        out.insert("gap".to_string(), Value::String(gap.clone()));
    }
    if let Some(padding) = layout.padding.as_ref() {
        out.insert("padding".to_string(), Value::String(padding.clone()));
    }
    if let Some(align) = layout.align.as_ref() {
        out.insert("align".to_string(), Value::String(align.clone()));
    }
    if let Some(justify) = layout.justify.as_ref() {
        out.insert("justify".to_string(), Value::String(justify.clone()));
    }
    Value::Object(out)
}
